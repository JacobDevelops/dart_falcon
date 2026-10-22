//! Shared large-stack workers for parsing and analysis.

use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::thread;

use rayon::{ThreadPool, ThreadPoolBuilder};

// The parser's depth ceiling needs under 16 MB; analysis also recursively walks
// legal trees, so retain the 256 MB safety margin for every worker.
const STACK_SIZE: usize = 256 * 1024 * 1024;
static POOL: RetryCell<ThreadPool> = RetryCell::new();

fn build_pool() -> io::Result<ThreadPool> {
    ThreadPoolBuilder::new()
        .stack_size(STACK_SIZE)
        .thread_name(|index| format!("falcon-stack-{index}"))
        .build()
        .map_err(io::Error::other)
}

/// Keeps the first successful build; a failed build is retried by the next caller.
struct RetryCell<T> {
    value: OnceLock<T>,
    building: Mutex<()>,
}

impl<T> RetryCell<T> {
    const fn new() -> Self {
        Self {
            value: OnceLock::new(),
            building: Mutex::new(()),
        }
    }

    fn get_or_build<E>(&self, build: impl FnOnce() -> Result<T, E>) -> Result<&T, E> {
        if let Some(value) = self.value.get() {
            return Ok(value);
        }
        // Serializes builds so concurrent first callers don't each start a pool.
        let _building = self.building.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(value) = self.value.get() {
            return Ok(value);
        }
        let value = build()?;
        Ok(self.value.get_or_init(|| value))
    }
}

/// Run parsing or analysis on the shared large-stack Rayon pool.
///
/// Nested calls on this pool run directly, without scheduling another job.
/// Workers of other Rayon pools block until the operation finishes rather than
/// running their own queued jobs on their smaller stacks meanwhile.
///
/// # Errors
///
/// Returns an error if the pool or a waiting thread cannot start, or the
/// operation panics. A later call retries starting the pool. A panic abandons
/// the operation's output; callers must not reuse partially mutated state.
pub fn with_stack<T: Send>(operation: impl FnOnce() -> T + Send) -> io::Result<T> {
    let pool = POOL.get_or_build(build_pool)?;
    let result = if pool.current_thread_index().is_some() {
        catch_unwind(AssertUnwindSafe(operation))
    } else if rayon::current_thread_index().is_some() {
        run_from_foreign_worker(pool, operation)?
    } else {
        catch_unwind(AssertUnwindSafe(|| pool.install(operation)))
    };
    result.map_err(|payload| {
        let message = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("non-string panic payload");
        io::Error::other(format!("stack worker panicked: {message}"))
    })
}

// Enough for a thread that only waits; the operation itself runs on the pool.
const WAITER_STACK_SIZE: usize = 256 * 1024;

// A foreign worker in `install` runs its queued jobs while waiting, nesting each wave job's wait.
fn run_from_foreign_worker<T: Send>(
    pool: &ThreadPool,
    operation: impl FnOnce() -> T + Send,
) -> io::Result<thread::Result<T>> {
    thread::scope(|scope| {
        thread::Builder::new()
            .stack_size(WAITER_STACK_SIZE)
            .spawn_scoped(scope, || pool.install(operation))
            .map(|waiter| waiter.join())
            .map_err(|error| {
                io::Error::new(error.kind(), format!("starting stack waiter: {error}"))
            })
    })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use rayon::prelude::*;

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
        let pool = POOL.get_or_build(build_pool).unwrap();
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

    #[test]
    fn failed_builds_are_retried_and_success_is_reused() {
        let cell = RetryCell::new();
        let mut attempts = 0;
        let mut build = || {
            attempts += 1;
            if attempts == 1 {
                Err("transient")
            } else {
                Ok(attempts)
            }
        };
        assert_eq!(cell.get_or_build(&mut build), Err("transient"));
        assert_eq!(cell.get_or_build(&mut build), Ok(&2));
        assert_eq!(cell.get_or_build(&mut build), Ok(&2));
        assert_eq!(attempts, 2);
    }

    #[test]
    fn foreign_workers_block_without_running_their_queued_jobs() {
        thread_local!(static WAITING: Cell<bool> = const { Cell::new(false) });
        // Single-item splits keep the worker's own deque full while it waits.
        let foreign = ThreadPoolBuilder::new().num_threads(1).build().unwrap();
        let items: Vec<usize> = (0..4096).collect();
        let doubled: Vec<usize> = foreign.install(|| {
            items
                .par_iter()
                .with_max_len(1)
                .map(|item| {
                    assert!(!WAITING.replace(true), "a waiting worker ran a queued job");
                    let doubled = with_stack(|| {
                        let worker = std::thread::current();
                        assert!(
                            worker
                                .name()
                                .is_some_and(|name| name.starts_with("falcon-stack-"))
                        );
                        item * 2
                    })
                    .unwrap();
                    WAITING.set(false);
                    doubled
                })
                .collect()
        });
        assert_eq!(
            doubled,
            items.iter().map(|item| item * 2).collect::<Vec<_>>()
        );
        let error = foreign
            .install(|| with_stack(|| panic!("foreign panic")))
            .unwrap_err();
        assert!(error.to_string().contains("foreign panic"));
    }
}
