//! Where turbomap's CPU work runs: tile decode, the DEM codec and MVT
//! tessellation. **The host owns the threads.** The engine never spawns
//! one: it hands each job to the [`Executor`] it was built with, so an
//! application that already has a work pool (an editor's job system, a
//! game's task graph) runs the map's decodes there, under its own
//! priorities, instead of beside them on threads it cannot see.
//!
//! A host with no pool of its own uses [`ThreadPool`], which is what the
//! engine used to spawn for itself.
//!
//! wasm has no threads. There the engine decodes inline inside its
//! per-frame apply budget, and takes no executor.

/// One unit of CPU work.
pub type Job = Box<dyn FnOnce() + Send + 'static>;

/// Runs [`Job`]s off the calling thread.
///
/// Every job handed to [`Executor::spawn`] must eventually run. The engine
/// does not trust this: a job that is dropped without running (a pool that
/// shut down, a queue that discarded it, a job that panicked) is detected
/// by the engine on its next frame, which then panics naming the tile. A
/// lost decode would otherwise leave the tile pending forever, and a
/// render-on-demand host would spin waiting for it.
pub trait Executor: Send + Sync {
    fn spawn(&self, job: Job);
}

/// A fixed set of named threads draining one queue: the executor for a
/// host that has none of its own.
#[cfg(not(target_arch = "wasm32"))]
pub struct ThreadPool {
    jobs: std::sync::mpsc::Sender<Job>,
}

#[cfg(not(target_arch = "wasm32"))]
impl ThreadPool {
    /// `threads` workers named `{name}-{i}`. A thread that cannot be
    /// spawned is fatal and names the pool: a map that silently decodes
    /// nothing is worse than one that does not start.
    pub fn new(name: &str, threads: std::num::NonZeroUsize) -> Self {
        let (jobs, rx) = std::sync::mpsc::channel::<Job>();
        let rx = std::sync::Arc::new(std::sync::Mutex::new(rx));
        for i in 0..threads.get() {
            let rx = rx.clone();
            std::thread::Builder::new()
                .name(format!("{name}-{i}"))
                .spawn(move || loop {
                    // Poisoning means another worker panicked while holding
                    // the receiver: a bug, never retried.
                    let next = rx
                        .lock()
                        .unwrap_or_else(|_| panic!("turbomap: a worker of this pool panicked holding its queue (a bug)"))
                        .recv();
                    match next {
                        Ok(job) => job(),
                        // The pool was dropped: every sender is gone.
                        Err(_) => break,
                    }
                })
                .unwrap_or_else(|e| panic!("turbomap: the {name} pool could not spawn thread {i}: {e}"));
        }
        Self { jobs }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Executor for ThreadPool {
    fn spawn(&self, job: Job) {
        // The workers hold the receiver until the pool drops, and the pool
        // is alive here, so a send fails only if every worker has died,
        // i.e. panicked. That is a bug to surface, not a job to drop.
        if self.jobs.send(job).is_err() {
            panic!("turbomap: every worker of this pool has died (a job panicked)");
        }
    }
}
