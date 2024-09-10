use std::{
    cell::UnsafeCell,
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Progress {
    Queued,
    InProgress(f32),
    Finished,
}

pub struct Checker {
    inner: Arc<Mutex<Progress>>,
    buffer: UnsafeCell<Progress>,
}

pub struct Updater {
    inner: Arc<Mutex<Progress>>,
}

impl Checker {
    pub fn check_progress(&self) -> Progress {
        // SAFETY:
        // Because `ProgressChecker` is `!Sync`, all calls to `check_progress` (the only place
        // where the `UnsafeCell` is accessed) will be synchronous and exclusive access is guaranteed.
        if let Ok(progress) = self.inner.try_lock() {
            unsafe {
                *(self.buffer.get()) = *progress;
            }
        } else {
            tracing::info!("Is there actually any lock contention?");
        }

        unsafe { *self.buffer.get() }
    }
}

impl Updater {
    pub fn update_progress(&mut self, progress: Progress) {
        *self.inner.lock().expect("Epic fail") = progress;
    }
}

#[must_use]
pub fn create_pair() -> (Updater, Checker) {
    let inner = Arc::new(Mutex::new(Progress::Queued));

    (
        Updater {
            inner: inner.clone(),
        },
        Checker {
            inner,
            buffer: UnsafeCell::new(Progress::Queued),
        },
    )
}
