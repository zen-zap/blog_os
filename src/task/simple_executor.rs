use super::Task;
use alloc::collections::VecDeque;
use alloc::vec::Vec;

/// helps in managing the Futures
pub struct SimpleExecutor {
	/// Simple FIFO queue
	task_queue: VecDeque<Task>,
}

impl SimpleExecutor {
	pub fn new() -> SimpleExecutor {
		SimpleExecutor { task_queue: VecDeque::new() }
	}

	pub fn spawn(
		&mut self,
		task: Task,
	) {
		self.task_queue.push_back(task)
	}

	/// Doesn't really utilize the Waker type
	pub fn run(&mut self) {
		while let Some(mut task) = self.task_queue.pop_front() {
			let waker = dummy_waker();
			let mut context = Context::from_waker(&waker);
			match task.poll(&mut context) {
				Poll::Ready(()) => {},
				// if the task is unfinished then add it to the back of the queue again
				Poll::Pending => self.task_queue.push_back(task),
			}
		}
	}
}

use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

/// RawWaker requires the Programmer to explicitly define a virtual method table
///
/// This is a dummy RawWaker .. we don't really want to do anything here
///
/// Returns a RawWaker
fn dummy_raw_waker() -> RawWaker {
	fn no_op(_: *const ()) {}

	fn clone(_: *const ()) -> RawWaker {
		dummy_raw_waker()
	}

	let vtable = &RawWakerVTable::new(clone, no_op, no_op, no_op);

	RawWaker::new(0 as *const (), vtable)
}

fn dummy_waker() -> Waker {
	unsafe { Waker::from_raw(dummy_raw_waker()) }
}
