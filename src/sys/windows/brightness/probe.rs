use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

/// Budget for DDC/CI and dxva2 probe operations.
pub(crate) const DDC_BUDGET: Duration = Duration::from_millis(500);

/// A boxed probe result: the closure's full `Result<Option<T>, String>`,
/// downcast back to `T` by the caller.
type ProbeResult = Box<dyn Any + Send>;

/// A probe job: the closure plus the per-call response channel, so results
/// are never cross-delivered between concurrent callers.
type ProbeJob = (Box<dyn FnOnce() -> ProbeResult + Send>, mpsc::Sender<ProbeResult>);

/// The shared probe worker: one thread running DDC/dxva2 probes
/// sequentially, started on first use. `busy` is the caller-side gate: a
/// claim precedes every queued job, so a hung or dead worker redirects
/// later calls to fresh threads instead of queueing behind the stuck job.
struct ProbeWorker {
    tx: mpsc::Sender<ProbeJob>,
    busy: Arc<AtomicBool>,
}

/// The lazily-started worker shared by every `timed` call.
static WORKER: OnceLock<ProbeWorker> = OnceLock::new();

/// The worker loop: runs each job, replies on its per-call channel, and
/// releases the busy claim. A hung closure leaves the claim held; a
/// panicking closure kills the thread with the claim still held, so all
/// later calls take the fresh-thread fallback.
fn worker_loop(rx: mpsc::Receiver<ProbeJob>, busy: Arc<AtomicBool>) {
    while let Ok((job, response_tx)) = rx.recv() {
        let result = job();
        let _ = response_tx.send(result);
        busy.store(false, Ordering::Release);
    }
}

/// Runs `f` with a time budget.
/// Returns the closure's result if it completes within `budget`,
/// otherwise returns `Ok(None)` (treated as "backend unsupported").
/// A panicking closure also yields `Ok(None)`.
///
/// An idle worker runs `f` on the shared probe thread; a busy worker (a
/// previous probe hung or panicked) spawns a fresh thread instead, so the
/// probe still runs concurrently with independent monitor handles.
pub(crate) fn timed<T, F>(budget: Duration, f: F) -> Result<Option<T>, String>
where
    F: FnOnce() -> Result<Option<T>, String> + Send + 'static,
    T: Send + 'static,
{
    let worker = WORKER.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        let busy = Arc::new(AtomicBool::new(false));
        thread::spawn({
            let busy = Arc::clone(&busy);
            move || worker_loop(rx, busy)
        });
        ProbeWorker { tx, busy }
    });
    if worker
        .busy
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return timed_on_fresh_thread(budget, f);
    }
    let (response_tx, response_rx) = mpsc::channel();
    let job: ProbeJob = (Box::new(move || -> ProbeResult { Box::new(f()) }), response_tx);
    if worker.tx.send(job).is_err() {
        worker.busy.store(false, Ordering::Release);
        return Ok(None);
    }
    match response_rx.recv_timeout(budget) {
        Ok(result) => match result.downcast::<Result<Option<T>, String>>() {
            Ok(inner) => *inner,
            Err(_) => Ok(None),
        },
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
        Err(mpsc::RecvTimeoutError::Disconnected) => Ok(None),
    }
}

/// Runs `f` on a spawned thread with a time budget: the fallback used
/// while the worker is busy or dead. Same semantics as the worker path —
/// a fresh thread with independent monitor handles.
pub(crate) fn timed_on_fresh_thread<T, F>(budget: Duration, f: F) -> Result<Option<T>, String>
where
    F: FnOnce() -> Result<Option<T>, String> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(f());
    });
    match rx.recv_timeout(budget) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
        Err(mpsc::RecvTimeoutError::Disconnected) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timed_fast_closure_returns_result() {
        use std::time::Duration;
        let result = timed(Duration::from_millis(50), || Ok(Some(true)));
        assert_eq!(result, Ok(Some(true)));
    }

    #[test]
    fn timed_slow_closure_returns_none() {
        use std::time::Duration;
        let result = timed(Duration::from_millis(30), || {
            std::thread::sleep(Duration::from_millis(100));
            Ok(Some(false))
        });
        assert_eq!(result, Ok(None));
    }

    #[test]
    fn timed_panicking_closure_returns_none() {
        use std::time::Duration;
        let result: Result<Option<bool>, String> = timed(Duration::from_millis(50), || {
            panic!("intentional panic");
        });
        assert_eq!(result, Ok(None));
    }

    #[test]
    fn timed_busy_worker_falls_back_to_a_fresh_thread() {
        use std::time::Duration;
        let (started_tx, started_rx) = mpsc::channel::<()>();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let blocker = std::thread::spawn(move || {
            let result = timed(Duration::from_millis(2000), move || {
                let _ = started_tx.send(());
                let _ = release_rx.recv();
                Ok(Some(true))
            });
            assert_eq!(result, Ok(Some(true)));
        });
        started_rx.recv_timeout(Duration::from_millis(1000)).unwrap();
        let result = timed(Duration::from_millis(50), || Ok(Some(42)));
        assert_eq!(result, Ok(Some(42)));
        let _ = release_tx.send(());
        blocker.join().unwrap();
    }
}