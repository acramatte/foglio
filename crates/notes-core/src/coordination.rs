//! Bounded coordination for operations sharing one Library handle. This does
//! not replace or retry the nonblocking inter-process filesystem lock.
use crate::{Error, ErrorCode, Result};
use std::{
    sync::{Arc, Condvar, Mutex},
    thread::{self, ThreadId},
    time::{Duration, Instant},
};

#[derive(Debug, Default)]
pub(crate) struct Gate {
    owner: Mutex<Option<ThreadId>>,
    released: Condvar,
}
#[derive(Debug)]
pub(crate) struct Permit(Arc<Gate>);
impl Gate {
    pub(crate) fn acquire(self: &Arc<Self>) -> Result<Permit> {
        let deadline = Instant::now() + Duration::from_secs(2);
        let current = thread::current().id();
        let mut owner = self.owner.lock().unwrap();
        while let Some(active) = *owner {
            if active == current {
                return Err(Error::new(
                    ErrorCode::Busy,
                    "library operation is not reentrant",
                ));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(Error::new(
                    ErrorCode::Busy,
                    "local library operation still running; retry explicitly",
                ));
            }
            owner = self.released.wait_timeout(owner, remaining).unwrap().0;
        }
        *owner = Some(current);
        Ok(Permit(self.clone()))
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        *self.0.owner.lock().unwrap() = None;
        self.0.released.notify_one();
    }
}
