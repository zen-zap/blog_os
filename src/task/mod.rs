//! in src/task/mod.rs
//!
//! Contains the struct for Task and TaskMetadata along with implementations

pub mod executor;
pub mod keyboard;
pub mod lock;
pub mod pinh;
pub mod simple_executor;

use alloc::{boxed::Box, vec::Vec};
use core::{
	future::Future,
	pin::Pin,
	task::{Context, Poll},
};

/// contains metadata for any tasks created
///
/// tracks priority and locks held by the task
struct TaskMetadata {
	/// base priority
	base_priority: u8,
	/// dynamic priority -- we can boost or decay this
	dyn_priority: u8,
	/// list of Lock IDs held by the current task -- this is gonna help in dependency tracking
	locks_held: Vec<lock::LockId>,
}

/// Represents an asynchronous task to be executed by the executor.
/// A Task is an individual unit of work.
///
/// Each `Task` contains:
/// - a unique identifier (`TaskId`)
/// - a pinned, heap-allocated future to be polled
/// - associated metadata such as base and dynamic priority
///
/// The future is stored as a `Pin<Box<dyn Future<Output = ()>>>` to ensure it
/// is not moved in memory and can be safely polled across multiple calls.
pub struct Task {
	/// unique identifier
	id: TaskId,
	/// future to be executed
	future: Pin<Box<dyn Future<Output = ()>>>,
	// methods on the Future are dynamically dispatched
	/// metadata
	meta: TaskMetadata,
}

impl Task {
	/// Creates a new `Task` with the given priority and future.
	///
	/// # Arguments
	///
	/// * `priority` - The base and initial dynamic priority for the task.
	/// * `future` - An asynchronous computation to be executed by the task. Must be `'static` as it may live for the lifetime of the executor.
	///
	/// # Returns
	///
	/// A `Task` containing a unique identifier, the pinned future, and associated metadata.
	///
	/// The future is stored as a `Pin<Box<dyn Future<Output = ()>>>` to ensure it is not moved in memory and can be safely polled.
	pub fn new(
		priority: u8,
		future: impl Future<Output = ()> + 'static,
	) -> Task {
		Task {
			id: TaskId::new(), // makes it possible for uniquely naming a task for specific wake-ups
			future: Box::pin(future),
			meta: TaskMetadata {
				base_priority: priority,
				dyn_priority: priority,
				locks_held: Vec::new(),
			},
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

/// simple wrapper around u64 to hold TaskIDs
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskId(u64);

use crate::task::pinh::PriLock;
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

/*
Each task in its metadata cannot hold the entire copy of the entire PriLock,
When Task A acquires a lock and Task B wants it, then B would get its own copy of the PriLock.
If Task B adds itself to the waiters list, its adding itself to its own private copy.
Task A's copy of lock knows nothing of this new waiter. This leads to Stale State.
There is no single source of truth for who owns a lock or who is waiting on a lock.
 */
