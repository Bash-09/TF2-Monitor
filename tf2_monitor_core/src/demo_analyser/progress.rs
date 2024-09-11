use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Progress {
    Queued,
    InProgress(f32),
    Finished,
}

pub struct Checker {
    inner: Arc<Mutex<Progress>>,
}

pub struct Updater {
    inner: Arc<Mutex<Progress>>,
}

impl Checker {
    /// # Panics
    /// If the lock is poisoned
    #[must_use]
    pub fn check_progress(&self) -> Progress {
        *self.inner.lock().expect("Epic fail")
    }
}

impl Updater {
    /// # Panics
    /// If the lock is poisoned
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
        Checker { inner },
    )
}
