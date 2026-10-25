#![cfg(feature = "scheduler")]

use scafra_config::{ConfigLoader, ScafraConfig};
use scafra_scheduler::Scheduler;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
    time::Duration,
};

static NEXT: AtomicU64 = AtomicU64::new(0);
static RUNS: AtomicUsize = AtomicUsize::new(0);
fn tick() {
    RUNS.fetch_add(1, Ordering::SeqCst);
}
scafra_scheduler::register_scheduled_task!("MixedCase", 60000, tick);
scafra_scheduler::register_scheduled_task!("foo.bar", 60000, tick);
scafra_scheduler::register_scheduled_task!("with__gap", 60000, tick);
scafra_scheduler::register_scheduled_task!("case", 60000, tick);
scafra_scheduler::register_scheduled_task!("CASE", 60000, tick);
scafra_scheduler::register_scheduled_task!("name_hex_ab", 60000, tick);
scafra_scheduler::register_scheduled_task!("TASK_HEX_ab", 60000, tick);
scafra_scheduler::register_scheduled_task!(".", 60000, tick);
scafra_scheduler::register_scheduled_task!("foo..bar", 60000, tick);
const NAMES: &[&str] = &[
    "MixedCase",
    "foo.bar",
    "with__gap",
    "case",
    "CASE",
    "name_hex_ab",
    "TASK_HEX_ab",
    ".",
    "foo..bar",
];

struct Case {
    directory: PathBuf,
    prefix: String,
}
impl Case {
    fn new() -> Self {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/cfg111-task-name-tests")
            .join(format!("{}-{id}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        Self {
            directory,
            prefix: format!("CFG111_NAMES_{}_{id}", std::process::id()),
        }
    }
    fn loader(&self) -> ConfigLoader {
        ConfigLoader::new()
            .config_dir(&self.directory)
            .env_prefix(&self.prefix)
    }
    fn write(&self, name: &str, text: &str) {
        fs::write(self.directory.join(name), text).unwrap();
    }
}
impl Drop for Case {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}
struct Variable(String);
impl Variable {
    fn new(case: &Case, task: &str, field: &str, value: &str) -> Self {
        let name = format!("{}_SCHEDULER__TASKS__{task}__{field}", case.prefix);
        assert!(std::env::var_os(&name).is_none());
        std::env::set_var(&name, value);
        Self(name)
    }
}
impl Drop for Variable {
    fn drop(&mut self) {
        std::env::remove_var(&self.0);
    }
}
fn encoded(name: &str) -> String {
    format!(
        "TASK_HEX_{}",
        name.bytes()
            .map(|byte| format!("{byte:02X}"))
            .collect::<String>()
    )
}
fn assert_interval(config: &ScafraConfig, name: &str, expected: u64) {
    assert_eq!(config.scheduler.tasks.len(), 1, "{name}");
    assert_eq!(
        config.scheduler.tasks[name].interval_ms,
        Some(expected),
        "{name}"
    );
}

#[test]
fn literal_registration_names_round_trip_all_files_profiles_and_programmatic_paths() {
    for name in NAMES {
        for extension in ["yaml", "yml", "properties"] {
            let case = Case::new();
            let input = if extension == "properties" {
                format!("scheduler.tasks.{name}.interval_ms=25\n")
            } else {
                format!("scheduler:\n  tasks:\n    '{name}': {{interval_ms: 25}}\n")
            };
            let filename = format!("application.{extension}");
            case.write(&filename, &input);
            assert_interval(
                &case.loader().load_validated::<ScafraConfig>().unwrap(),
                name,
                25,
            );
            fs::rename(
                case.directory.join(filename),
                case.directory
                    .join(format!("application-production.{extension}")),
            )
            .unwrap();
            assert_interval(
                &case
                    .loader()
                    .profile("production")
                    .load_validated::<ScafraConfig>()
                    .unwrap(),
                name,
                25,
            );
        }
        let case = Case::new();
        let config = case
            .loader()
            .override_value(format!("scheduler.tasks.{name}.interval_ms"), "35")
            .load_validated::<ScafraConfig>()
            .unwrap();
        assert_interval(&config, name, 35);
    }
}

#[test]
fn encoded_environment_names_preserve_every_literal_task_name() {
    for name in NAMES {
        let case = Case::new();
        let _variable = Variable::new(&case, &encoded(name), "INTERVAL_MS", "45");
        let config = case.loader().load_validated::<ScafraConfig>().unwrap();
        assert_interval(&config, name, 45);
    }
}

#[test]
fn ordinary_environment_names_preserve_unique_registered_case_and_reject_collisions() {
    let case = Case::new();
    let variable = Variable::new(&case, "MIXEDCASE", "INTERVAL_MS", "55");
    assert_interval(
        &case.loader().load_validated::<ScafraConfig>().unwrap(),
        "MixedCase",
        55,
    );
    drop(variable);
    let legacy = format!("{}_SCHEDULER_TASKS_MIXEDCASE_INTERVAL_MS", case.prefix);
    std::env::set_var(&legacy, "65");
    let config = case.loader().load_validated::<ScafraConfig>();
    std::env::remove_var(legacy);
    assert_interval(&config.unwrap(), "MixedCase", 65);
    let variable = Variable::new(&case, "CASE", "INTERVAL_MS", "75");
    assert!(case.loader().load_validated::<ScafraConfig>().is_err());
    drop(variable);
    let _lower = Variable::new(&case, &encoded("case"), "INTERVAL_MS", "85");
    let _upper = Variable::new(&case, &encoded("CASE"), "INTERVAL_MS", "95");
    let config = case.loader().load_validated::<ScafraConfig>().unwrap();
    assert_eq!(config.scheduler.tasks["case"].interval_ms, Some(85));
    assert_eq!(config.scheduler.tasks["CASE"].interval_ms, Some(95));
}

#[test]
fn invalid_encoded_names_duplicate_paths_and_unknown_fields_fail_safely() {
    for token in [
        "TASK_HEX_",
        "TASK_HEX_0",
        "TASK_HEX_ZZ",
        "TASK_HEX_FF",
        "TASK_HEX_00",
    ] {
        let case = Case::new();
        let _variable = Variable::new(&case, token, "INTERVAL_MS", "do-not-echo-task-secret");
        let error = case.loader().load_validated::<ScafraConfig>().unwrap_err();
        assert!(!format!("{error:?} {error}").contains("do-not-echo-task-secret"));
    }
    let case = Case::new();
    let _ordinary = Variable::new(&case, "MIXEDCASE", "INTERVAL_MS", "25");
    let _encoded = Variable::new(&case, &encoded("MixedCase"), "INTERVAL_MS", "35");
    assert!(case.loader().load_validated::<ScafraConfig>().is_err());
    drop(_ordinary);
    drop(_encoded);
    for path in [
        "scheduler.tasks.foo.bar.typo",
        "scheduler.tasks.MixedCase.interval_ms.typo",
    ] {
        assert!(case
            .loader()
            .override_value(path, "35")
            .load_validated::<ScafraConfig>()
            .is_err());
    }
    case.write(
        "application.yaml",
        "scheduler: {tasks: {'foo.bar': {typo: 35}}}\n",
    );
    assert!(case.loader().load_validated::<ScafraConfig>().is_err());
}

#[tokio::test]
async fn encoded_environment_interval_changes_actual_scheduler_execution() {
    for name in NAMES {
        let case = Case::new();
        let _variable = Variable::new(&case, &encoded(name), "INTERVAL_MS", "2");
        let config = case
            .loader()
            .override_value("scheduler.enabled", "true")
            .load_validated::<ScafraConfig>()
            .unwrap();
        RUNS.store(0, Ordering::SeqCst);
        let scheduler = Scheduler::try_start(&config.scheduler).unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while RUNS.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        scheduler.shutdown().await;
    }
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
struct DomainSettings {
    scheduler: DomainScheduler,
}
#[derive(Default, serde::Deserialize, serde::Serialize)]
struct DomainScheduler {
    tasks: DomainTasks,
}
#[derive(Default, serde::Deserialize, serde::Serialize)]
struct DomainTasks {
    foo: DomainFoo,
    case: DomainTask,
    task_hex_ab: DomainTask,
}
#[derive(Default, serde::Deserialize, serde::Serialize)]
struct DomainFoo {
    retry: DomainLeaf,
    interval_ms: DomainLeaf,
}
#[derive(Default, serde::Deserialize, serde::Serialize)]
struct DomainLeaf {
    max: u64,
}
#[derive(Default, serde::Deserialize, serde::Serialize)]
struct DomainTask {
    interval_ms: u64,
}

#[test]
fn domain_loader_keeps_ordinary_nested_paths_and_environment_names() {
    let case = Case::new();
    case.write(
        "application.properties",
        "scheduler.tasks.foo.retry.max=7\nscheduler.tasks.foo.interval_ms.max=8\n",
    );
    let from_file = case.loader().load::<DomainSettings>().unwrap();
    assert_eq!(from_file.scheduler.tasks.foo.retry.max, 7);
    assert_eq!(from_file.scheduler.tasks.foo.interval_ms.max, 8);
    let explicit = case
        .loader()
        .override_value("scheduler.tasks.foo.retry.max", "9")
        .override_value("scheduler.tasks.foo.interval_ms.max", "10")
        .load::<DomainSettings>()
        .unwrap();
    assert_eq!(explicit.scheduler.tasks.foo.retry.max, 9);
    assert_eq!(explicit.scheduler.tasks.foo.interval_ms.max, 10);
    // These are domain fields even though linked framework tasks collide by
    // case or resemble the scheduler-only hex escape prefix.
    let _case = Variable::new(&case, "CASE", "INTERVAL_MS", "11");
    let _hex = Variable::new(&case, "TASK_HEX_AB", "INTERVAL_MS", "12");
    let environment = case.loader().load::<DomainSettings>().unwrap();
    assert_eq!(environment.scheduler.tasks.case.interval_ms, 11);
    assert_eq!(environment.scheduler.tasks.task_hex_ab.interval_ms, 12);
}
