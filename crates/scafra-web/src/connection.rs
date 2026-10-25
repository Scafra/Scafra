//! Managed Hyper connections, bounded admission and transport inactivity.
use crate::{ServerError, ServerOutcome, WebError};
use axum::{extract::ConnectInfo, Extension, Router};
use hyper_util::{
    rt::{TokioIo, TokioTimer},
    server::conn::auto::Builder,
    service::TowerToHyperService,
};
use scafra_foundation::{ShutdownPolicy, ShutdownReason};
use std::{
    future::Future,
    io,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::{TcpListener, TcpStream},
    sync::{watch, OwnedSemaphorePermit, Semaphore},
    task::JoinSet,
    time::Instant,
};
use tower::Layer;

/// Connection controls owned by the managed server, not router middleware.
#[derive(Debug, Clone)]
pub struct ConnectionOptions {
    /// HTTP/1 deadline to complete request headers.
    pub header_read_timeout: Duration,
    /// Maximum interval without successful transport read/write progress.
    pub idle_timeout: Duration,
    pub max_connections: usize,
    /// HTTP/1 parser buffer ceiling; HTTP/2 maximum decoded header list bytes.
    pub max_header_bytes: usize,
    pub http1_keep_alive: bool,
    pub http1_max_headers: usize,
    pub http2_max_concurrent_streams: u32,
}
impl Default for ConnectionOptions {
    fn default() -> Self {
        Self {
            header_read_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(60),
            max_connections: 1024,
            max_header_bytes: 32768,
            http1_keep_alive: true,
            http1_max_headers: 100,
            http2_max_concurrent_streams: 200,
        }
    }
}
impl ConnectionOptions {
    pub fn validate(&self) -> Result<(), WebError> {
        if self.header_read_timeout.is_zero()
            || self.header_read_timeout > Duration::from_secs(3600)
            || self.idle_timeout.is_zero()
            || self.idle_timeout > Duration::from_secs(86400)
            || !(1..=1_000_000).contains(&self.max_connections)
            || !(8192..=1_048_576).contains(&self.max_header_bytes)
            || !(1..=1000).contains(&self.http1_max_headers)
            || !(1..=10000).contains(&self.http2_max_concurrent_streams)
        {
            return Err(WebError::Server(io::Error::new(io::ErrorKind::InvalidInput,
                "invalid connection options: header deadline 0<duration<=1h, inactivity 0<duration<=1d, connections 1..1000000, header bytes 8192..1048576, HTTP/1 header fields 1..1000, HTTP/2 streams 1..10000")));
        }
        Ok(())
    }
}

struct Activity {
    start: Instant,
    last_progress_nanos: AtomicU64,
}
impl Activity {
    fn progress(&self) {
        let elapsed = self.start.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        self.last_progress_nanos.store(elapsed, Ordering::Relaxed);
    }
    async fn expired(&self, idle: Duration) {
        loop {
            let last =
                self.start + Duration::from_nanos(self.last_progress_nanos.load(Ordering::Relaxed));
            let deadline = last + idle;
            if Instant::now() >= deadline {
                return;
            }
            tokio::time::sleep_until(deadline).await;
        }
    }
}
struct IdleIo {
    stream: TcpStream,
    activity: Arc<Activity>,
    // Hyper transfers IO on upgrade; retain admission ownership until the
    // upgraded socket is actually released by the application.
    _permit: OwnedSemaphorePermit,
}

impl AsyncRead for IdleIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let before = buf.filled().len();
        let result = Pin::new(&mut self.stream).poll_read(cx, buf);
        if matches!(result, Poll::Ready(Ok(()))) && buf.filled().len() > before {
            self.activity.progress();
        }
        result
    }
}
impl AsyncWrite for IdleIo {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let result = Pin::new(&mut self.stream).poll_write(cx, buf);
        if matches!(result, Poll::Ready(Ok(n)) if n > 0) {
            self.activity.progress();
        }
        result
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConnectionCommand {
    Running,
    Drain,
    Force,
}

// Hyper executes HTTP/2 stream futures independently of the connection driver.
// Track them in a connection-local scope so resource cleanup cannot race a
// still-running handler after a forced drain. HTTP/1 handlers are polled by the
// driver itself. Completed tasks are reaped on each spawn to bound bookkeeping.
#[derive(Clone)]
struct ConnectionExecutor {
    tasks: Arc<Mutex<Option<JoinSet<()>>>>,
}
impl<F> hyper::rt::Executor<F> for ConnectionExecutor
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    fn execute(&self, future: F) {
        let mut scope = self
            .tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(tasks) = scope.as_mut() {
            while tasks.try_join_next().is_some() {}
            tasks.spawn(async move {
                let _ = future.await;
            });
        }
        // A closed scope drops the future instead of detaching new work.
    }
}
struct ConnectionTasks(ConnectionExecutor);
impl ConnectionTasks {
    fn new() -> Self {
        Self(ConnectionExecutor {
            tasks: Arc::new(Mutex::new(Some(JoinSet::new()))),
        })
    }
    async fn shutdown(self) {
        let tasks = self
            .0
            .tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(mut tasks) = tasks {
            tasks.shutdown().await;
        }
    }
}
impl Drop for ConnectionTasks {
    fn drop(&mut self) {
        if let Some(tasks) = self
            .0
            .tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_mut()
        {
            tasks.abort_all();
        }
    }
}

pub(crate) async fn serve<F>(
    listener: TcpListener,
    router: Router,
    shutdown: F,
    policy: ShutdownPolicy,
    options: ConnectionOptions,
) -> Result<ServerOutcome, ServerError>
where
    F: Future<Output = Result<ShutdownReason, WebError>> + Send + 'static,
{
    options.validate()?;
    let (drain, _) = watch::channel(ConnectionCommand::Running);
    let mut connections = JoinSet::new();
    tokio::pin!(shutdown);
    let capacity = Arc::new(Semaphore::new(options.max_connections));
    let mut accept_ready = Instant::now();
    let trigger = loop {
        tokio::select! {
            biased;
            trigger = &mut shutdown => break trigger,
            _ = connections.join_next(), if !connections.is_empty() => {},
            accepted = async {
                tokio::time::sleep_until(accept_ready).await;
                let permit = Arc::clone(&capacity).acquire_owned().await.map_err(|_| io::Error::other("connection capacity unavailable"))?;
                listener.accept().await.map(|accepted| (accepted, permit))
            }, if connections.len() < options.max_connections => {
                let ((stream, address), permit) = match accepted {
                    Ok(accepted) => accepted,
                    Err(_) => {
                        // Peer aborts and descriptor exhaustion are recoverable.
                        // Back off without blocking shutdown or active connections.
                        scafra_foundation::warn!(error_kind = "listener_accept", "connection acceptance failed; retrying");
                        accept_ready = Instant::now() + Duration::from_secs(1);
                        continue;
                    }
                };
                let service = TowerToHyperService::new(Extension(ConnectInfo(address)).layer(router.clone()));
                let options = options.clone();
                let mut shutdown = drain.subscribe();
                connections.spawn(async move {
                    let tasks = ConnectionTasks::new();
                    let mut builder = Builder::new(tasks.0.clone());
                    builder.http1().timer(TokioTimer::new())
                        .header_read_timeout(options.header_read_timeout)
                        .max_buf_size(options.max_header_bytes)
                        .keep_alive(options.http1_keep_alive)
                        .max_headers(options.http1_max_headers);
                    builder.http2().max_header_list_size(options.max_header_bytes as u32)
                        .max_concurrent_streams(options.http2_max_concurrent_streams);
                    let activity = Arc::new(Activity { start: Instant::now(), last_progress_nanos: AtomicU64::new(0) });
                    {
                        let connection = builder.serve_connection_with_upgrades(
                            TokioIo::new(IdleIo { stream, activity: Arc::clone(&activity), _permit: permit }), service);
                        tokio::pin!(connection);
                        tokio::select! {
                            biased;
                            _ = activity.expired(options.idle_timeout) => {},
                            command = shutdown.changed() => {
                                if command.is_ok() && *shutdown.borrow() == ConnectionCommand::Drain {
                                    connection.as_mut().graceful_shutdown();
                                    tokio::select! {
                                        biased;
                                        _ = shutdown.changed() => {},
                                        _ = activity.expired(options.idle_timeout) => {},
                                        _ = &mut connection => {},
                                    }
                                }
                            }
                            _ = &mut connection => {}
                        }
                    } // Drop the driver before closing its HTTP/2 task scope.
                    tasks.shutdown().await;
                    // Transport/parser errors are connection-local and never
                    // formatted: peer data and URIs can contain credentials.
                });
            }
        }
    };
    drop(listener);
    let reason = trigger
        .as_ref()
        .copied()
        .unwrap_or(ShutdownReason::RuntimeFailure);
    let _ = drain.send(ConnectionCommand::Drain);
    let drain_connections = async { while connections.join_next().await.is_some() {} };
    if policy.force_after_grace() {
        if tokio::time::timeout(policy.grace_period(), drain_connections)
            .await
            .is_err()
        {
            let _ = drain.send(ConnectionCommand::Force);
            while connections.join_next().await.is_some() {}
            return Err(ServerError::GracefulShutdownTimeout { reason, policy });
        }
    } else {
        drain_connections.await;
    }
    trigger
        .map(|reason| ServerOutcome { reason })
        .map_err(ServerError::from)
}
