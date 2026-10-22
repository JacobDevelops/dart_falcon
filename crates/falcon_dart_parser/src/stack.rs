//! Shared large-stack workers for parsing and analysis.

use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::LazyLock;

use rayon::{ThreadPool, ThreadPoolBuilder};

// The parser's depth ceiling needs under 16 MB; analysis also recursively walks
// legal trees, so retain the 256 MB safety margin for every worker.
const STACK_SIZE: usize = 256 * 1024 * 1024;
static POOL: LazyLock<io::Result<ThreadPool>> = LazyLock::new(|| {
    ThreadPoolBuilder::new()
        .stack_size(STACK_SIZE)
        .thread_name(|index| format!("falcon-stack-{index}"))
        .build()
        .map_err(io::Error::other)
});

/// Run parsing or analysis on the shared large-stack Rayon pool.
///
/// Nested calls on this pool run directly, without scheduling another job.
///
/// # Errors
///
/// Returns an error if the pool cannot start or the operation panics. A panic
/// abandons the operation's output; callers must not reuse partially mutated state.
pub fn with_stack<T: Send>(operation: impl FnOnce() -> T + Send) -> io::Result<T> {
    let pool = POOL
        .as_ref()
        .map_err(|error| io::Error::new(error.kind(), error.to_string()))?;
    catch_unwind(AssertUnwindSafe(|| {
        if pool.current_thread_index().is_some() {
            operation()
        } else {
            pool.install(operation)
        }
    }))
    .map_err(|payload| {
        let message = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("non-string panic payload");
        io::Error::other(format!("stack worker panicked: {message}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_calls_reuse_the_current_worker() {
        with_stack(|| {
            let worker = std::thread::current().id();
            for _ in 0..32 {
                assert_eq!(with_stack(|| std::thread::current().id()).unwrap(), worker);
            }
        })
        .unwrap();
    }

    #[test]
    fn repeated_calls_reuse_pool_workers() {
        let pool = POOL.as_ref().unwrap();
        let workers: Vec<_> = pool.broadcast(|_| std::thread::current().id());
        for _ in 0..32 {
            assert!(workers.contains(&with_stack(|| std::thread::current().id()).unwrap()));
        }
    }

    #[test]
    fn panics_are_errors_on_entry_and_when_nested() {
        let error = with_stack(|| panic!("entry panic")).unwrap_err();
        assert!(error.to_string().contains("entry panic"));
        with_stack(|| {
            let error = with_stack(|| panic!("nested panic")).unwrap_err();
            assert!(error.to_string().contains("nested panic"));
        })
        .unwrap();
        assert_eq!(with_stack(|| 42).unwrap(), 42);
    }
}
