//! in src/task/lock.rs

use core::sync::atomic::{AtomicU64, Ordering};
#[derive(Debug, Copy, Clone, PartialOrd, PartialEq, Eq, Ord)]
pub struct LockId(u64);

impl LockId {
	/// returns a new LockId
	pub fn new() -> Self {
		// atomic counter to ensure every ID is unique
		static NEXT_ID: AtomicU64 = AtomicU64::new(0);

		LockId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
	}
}
