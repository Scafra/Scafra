use scafra::{bean, AsyncResource, ShutdownPolicy, ShutdownReason};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

pub static FAIL_SECOND: AtomicBool = AtomicBool::new(false);
pub static POLICIES: Mutex<Vec<(ShutdownReason, ShutdownPolicy)>> = Mutex::new(Vec::new());

pub struct First;
pub struct Second {
    _first: Arc<First>,
}
impl AsyncResource for First {
    type ShutdownError = std::convert::Infallible;
    async fn shutdown(
        &self,
        reason: ShutdownReason,
        policy: ShutdownPolicy,
    ) -> Result<(), Self::ShutdownError> {
        POLICIES.lock().unwrap().push((reason, policy));
        Ok(())
    }
}
impl AsyncResource for Second {
    type ShutdownError = std::convert::Infallible;
    async fn shutdown(
        &self,
        reason: ShutdownReason,
        policy: ShutdownPolicy,
    ) -> Result<(), Self::ShutdownError> {
        POLICIES.lock().unwrap().push((reason, policy));
        Ok(())
    }
}
#[bean]
pub async fn first() -> First {
    First
}
#[bean]
pub async fn second(first: Arc<First>) -> Result<Second, std::io::Error> {
    if FAIL_SECOND.load(Ordering::SeqCst) {
        return Err(std::io::Error::other("configured initialization failure"));
    }
    Ok(Second { _first: first })
}
