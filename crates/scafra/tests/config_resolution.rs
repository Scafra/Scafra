use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use scafra::web::axum::Router;
use scafra::{ConfigError, StartupError};
use std::sync::atomic::AtomicBool;
use tokio::sync::Mutex;

use scafra_bootui as _;

static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);
static ENVIRONMENT_LOCK: Mutex<()> = Mutex::const_new(());
static ROUTER_BUILDER_CALLED: AtomicBool = AtomicBool::new(false);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let id = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "scafra-startup-config-{}-{nonce}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn configuration_child_process() {
    let Some(mode) = std::env::var_os("SCFRA_CONFIG_TEST_MODE") else {
        return;
    };
    let mode = mode.to_string_lossy();
    match mode.as_ref() {
        "startup" => {
            let expected_port = std::env::var("SCAFRA_SERVER_PORT")
                .unwrap()
                .parse::<u16>()
                .unwrap();
            let result = scafra::run_with_router(|config| {
                ROUTER_BUILDER_CALLED.store(true, Ordering::Release);
                assert_eq!(config.server.port, expected_port);
                assert_eq!(config.server.request_timeout_seconds, Some(17));
                assert!(config.security.enabled);
                assert_eq!(
                    config.security.bearer_token.as_deref(),
                    Some("config-resolution-secret")
                );
                assert_eq!(config.bootui.enabled, cfg!(feature = "bootui"));
                Ok(Router::new())
            })
            .await;
            assert!(
                result.is_ok(),
                "configured server should shut down cleanly: {result:?}"
            );
        }
        "missing-file" => {
            ROUTER_BUILDER_CALLED.store(false, Ordering::Release);
            let result = scafra::run_with_router(|_| {
                ROUTER_BUILDER_CALLED.store(true, Ordering::Release);
                Ok(Router::new())
            })
            .await;
            assert!(matches!(
                result,
                Err(StartupError::Config(ConfigError::Read { source, .. }))
                    if source.kind() == std::io::ErrorKind::NotFound
            ));
            assert!(!ROUTER_BUILDER_CALLED.load(Ordering::Acquire));
        }
        other => panic!("unknown child process mode {other}"),
    }
}

#[test]
fn standard_startup_uses_explicit_configuration_outside_the_project_directory() {
    let _environment_lock = ENVIRONMENT_LOCK.blocking_lock();
    let temp = TempDir::new();
    let config_dir = temp.path().join("deployed-config");
    let unexpected_cwd = temp.path().join("unexpected-cwd");
    fs::create_dir(&config_dir).unwrap();
    fs::create_dir(&unexpected_cwd).unwrap();
    fs::write(
        config_dir.join("application.yaml"),
        "scafra:\n  profiles:\n    active: base\nserver:\n  host: 127.0.0.1\n  port: 8080\nstartup:\n  banner: false\n",
    )
    .unwrap();
    let bootui = if cfg!(feature = "bootui") {
        "bootui:\n  enabled: true\n  host: 127.0.0.1\n  local_only: true\n"
    } else {
        ""
    };
    fs::write(
        config_dir.join("application-production.yaml"),
        format!("server:\n  timeout: 17\nsecurity:\n  enabled: true\n  bearer_token: config-resolution-secret\n{bootui}"),
    )
    .unwrap();
    let (server_port, bootui_port) = unused_local_ports();
    let server_port_value = server_port.to_string();
    let bootui_port_value = bootui_port.to_string();
    let mut variables = vec![
        ("SCAFRA_CONFIG_DIR", config_dir.as_os_str()),
        ("SCAFRA_PROFILE", "production".as_ref()),
        ("SCAFRA_SERVER_PORT", server_port_value.as_ref()),
        ("SCAFRA_STARTUP_BANNER", "false".as_ref()),
    ];
    if cfg!(feature = "bootui") {
        variables.push(("SCAFRA_BOOTUI_PORT", bootui_port_value.as_ref()));
    }
    let mut child = spawn_child("startup", &unexpected_cwd, &variables);

    if cfg!(feature = "bootui") {
        let snapshot = wait_for_bootui_snapshot(&mut child, bootui_port);
        assert!(
            snapshot.contains(&format!("\"port\":{server_port}")),
            "{snapshot}"
        );
        assert!(
            snapshot.contains("\"profile\":\"production\""),
            "{snapshot}"
        );
        assert!(!snapshot.contains("config-resolution-secret"));
    } else {
        // The standard feature intentionally excludes BootUI. Verify the actual
        // configured HTTP listener instead of enabling an unavailable module.
        let address = SocketAddr::from(([127, 0, 0, 1], server_port));
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            assert!(
                child.try_wait().unwrap().is_none(),
                "server exited before readiness"
            );
            if let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(100))
            {
                stream
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                stream
                    .write_all(
                        b"GET /unknown HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
                    )
                    .unwrap();
                let mut response = String::new();
                stream.read_to_string(&mut response).unwrap();
                assert!(response.starts_with("HTTP/1.1 401"), "{response}");
                assert!(response.contains("application/json"), "{response}");
                assert!(response.contains("www-authenticate: Bearer"), "{response}");
                assert!(!response.contains("config-resolution-secret"));
                break;
            }
            assert!(Instant::now() < deadline, "server did not start in time");
            thread::sleep(Duration::from_millis(20));
        }
    }

    terminate_child(&mut child);
}

#[test]
fn standard_startup_fails_on_a_missing_explicit_file_before_building_the_router() {
    let _environment_lock = ENVIRONMENT_LOCK.blocking_lock();
    let temp = TempDir::new();
    let unexpected_cwd = temp.path().join("unexpected-cwd");
    fs::create_dir(&unexpected_cwd).unwrap();
    let missing_file = temp.path().join("missing-application.yaml");

    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "configuration_child_process", "--nocapture"])
        .current_dir(&unexpected_cwd)
        .env("SCFRA_CONFIG_TEST_MODE", "missing-file")
        .env("SCAFRA_CONFIG_FILE", &missing_file)
        .env_remove("SCAFRA_CONFIG_DIR")
        .env_remove("SCAFRA_PROFILE")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "missing-file child failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn spawn_child(mode: &str, cwd: &std::path::Path, variables: &[(&str, &std::ffi::OsStr)]) -> Child {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "configuration_child_process", "--nocapture"])
        .current_dir(cwd)
        .env("SCFRA_CONFIG_TEST_MODE", mode)
        .env_remove("SCAFRA_CONFIG_FILE")
        .env_remove("SCAFRA_CONFIG_DIR")
        .env_remove("SCAFRA_PROFILE")
        .env_remove("SCAFRA_SERVER_HOST")
        .env_remove("SCAFRA_SERVER_PORT")
        .env_remove("SCAFRA_BOOTUI_PORT")
        .env_remove("SCAFRA_STARTUP_BANNER")
        .env_remove("SCAFRA_LOGGING_LEVEL")
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    for (key, value) in variables {
        command.env(key, value);
    }
    command.spawn().unwrap()
}

fn unused_local_ports() -> (u16, u16) {
    let first = TcpListener::bind("127.0.0.1:0").unwrap();
    let second = TcpListener::bind("127.0.0.1:0").unwrap();
    let first_port = first.local_addr().unwrap().port();
    let second_port = second.local_addr().unwrap().port();
    drop((first, second));
    (first_port, second_port)
}

fn wait_for_bootui_snapshot(child: &mut Child, port: u16) -> String {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let mut stderr = String::new();
            if let Some(mut stderr_pipe) = child.stderr.take() {
                let _ = stderr_pipe.read_to_string(&mut stderr);
            }
            panic!("startup child exited with {status}: {stderr}");
        }
        if let Some(snapshot) = request_snapshot(address) {
            return snapshot;
        }
        assert!(
            Instant::now() < deadline,
            "BootUI did not become available before the startup deadline"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

fn request_snapshot(address: SocketAddr) -> Option<String> {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(100)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(1))).ok()?;
    stream
        .write_all(
            b"GET /bootui/api/config HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_owned())
}

fn terminate_child(child: &mut Child) {
    #[cfg(unix)]
    {
        let status = Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .status()
            .unwrap();
        assert!(status.success(), "could not request child shutdown");
    }
    #[cfg(not(unix))]
    child.kill().unwrap();

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "startup child exited with {status}");
            return;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            panic!("startup child did not stop after a shutdown request");
        }
        thread::sleep(Duration::from_millis(20));
    }
}
