use std::sync::atomic::{AtomicBool, AtomicU64};


pub static PLAYOUTS: AtomicU64 = AtomicU64::new(0);
pub static STOP_SEARCH: AtomicBool = AtomicBool::new(false);
