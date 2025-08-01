// in src/task/executor.rs

use super::{Task, TaskId};
use alloc::{collections::BTreeMap, sync::Arc};
use core::task::{Context, Poll, Waker};
use crossbeam_queue::ArrayQueue;
use futures_util::task::waker;

pub struct Executor {
	tasks: BTreeMap<TaskId, Task>,
	/// reference counted ArrayQueue, shared between Executors and Wakers
	task_queue: Arc<ArrayQueue<TaskId>>,
	waker_cache: BTreeMap<TaskId, Waker>,
}

impl Executor {
	pub fn new() -> Self {
		Executor {
			tasks: BTreeMap::new(),
			// using a fixed queue, since interrupt handlers should not allocate on push
			task_queue: Arc::new(ArrayQueue::new(100)),
			waker_cache: BTreeMap::new(),
		}
	}

	pub fn spawn(
		&mut self,
		task: Task,
	) {
		let task_id = task.id;
		if self.tasks.insert(task.id, task).is_some() {
			panic!("task with same ID already in tasks");
		}
		self.task_queue.push(task_id).expect("queue full");
	}

	pub fn run(&mut self) -> ! {
		loop {
			self.run_ready_tasks();
		}
	}

	/// To execute all tasks in the task_queue
	///
	/// Loop over all tasks in the task_queue, create a waker for each task and then poll them
	fn run_ready_tasks(&mut self) {
		// destructure 'self' to avoid borrow checker errors
		let Self { tasks, task_queue, waker_cache } = self;

		while let Some(task_id) = task_queue.pop() {
			let task = match tasks.get_mut(&task_id) {
				Some(task) => task,
				None => continue,
			};
			let waker = waker_cache
				.entry(task_id)
				.or_insert_with(|| TaskWaker::new(task_id, task_queue.clone()));
			let mut context = Context::from_waker(waker);
			match task.poll(&mut context) {
				Poll::Ready(()) => {
					// task done -> remove it and its cached waker
					tasks.remove(&task_id);
					waker_cache.remove(&task_id);
				},
				Poll::Pending => {},
			}
		}
	}

	/// save power when no tasks are available
	///
	/// CPU put to sleep
	fn sleep_if_idle(&self) {
		use x86_64::instructions::interrupts::{self, enable_and_hlt};

		interrupts::disable();

		if self.task_queue.is_empty() {
			enable_and_hlt();
		} else {
			interrupts::enable();
		}
	}
}

struct TaskWaker {
	task_id: TaskId,
	task_queue: Arc<ArrayQueue<TaskId>>,
}

impl TaskWaker {
	fn new(
		task_id: TaskId,
		task_queue: Arc<ArrayQueue<TaskId>>,
	) -> Waker {
		Waker::from(Arc::new(TaskWaker { task_id, task_queue }))
	}

	fn wake_task(&self) {
		self.task_queue.push(self.task_id).expect("task_queue full");
	}
}

use alloc::task::Wake;
use x86_64::instructions::hlt;

// gotta convert our TaskWaker to a Waker instance first
// could also be done by using RawWaker
impl Wake for TaskWaker {
	fn wake(self: Arc<Self>) {
		self.wake_task();
	}

	fn wake_by_ref(self: &Arc<Self>) {
		self.wake_task();
	}
}
