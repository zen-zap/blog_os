// in src/task/mod.rs

pub mod executor;
pub mod keyboard;
pub mod simple_executor;

use alloc::boxed::Box;
use core::{
	future::Future,
	pin::Pin,
	task::{Context, Poll},
};

pub struct Task {
	id: TaskId,
	future: Pin<Box<dyn Future<Output = ()>>>,
	// methods on the Future are dynamically dispatched
}

impl Task {
	/// Pin<Box> type ensures that the value can never be moved in memory
	///
	/// It also prevents the creation of &mut references to it
	///
	/// The static lifetime is required because
	/// the Future can live for an arbitrary amount of time.
	pub fn new(future: impl Future<Output = ()> + 'static) -> Task {
		Task {
			id: TaskId::new(), // makes it possible for uniquely naming a task for specific wake-ups
			future: Box::pin(future),
		}
	}

	fn poll(
		&mut self,
		context: &mut Context,
	) -> Poll<()> {
		let _mutable_pinned_task_future = self.future.as_mut();
		self.future.as_mut().poll(context)
	}
}

/// simple wrapper around u64
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TaskId(u64);

use core::sync::atomic::{AtomicU64, Ordering};

impl TaskId {
	fn new() -> Self {
		// ensure each id is assigned only once
		static NEXT_ID: AtomicU64 = AtomicU64::new(0);
		// fetches the value and then increases it by one
		// Ordering defines the instruction sequence in the asm of fetch_add
		// Relaxed has the minimum requirements for reordering
		TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
	}
}
