use std::sync::{Mutex, MutexGuard, OnceLock};

pub(crate) fn color_control_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    LOCK.get_or_init(|| Mutex::new(())).lock().expect("color control test lock poisoned")
}
