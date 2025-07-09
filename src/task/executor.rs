// in src/task/executor.rs

use super::{Task, TaskId, TaskMetadata};
use alloc::{collections::BTreeMap, collections::BinaryHeap, sync::Arc};
use core::cmp::Reverse;
use core::task::{Context, Poll, Waker};
use crossbeam_queue::ArrayQueue;
use futures_util::task::waker;

/// Manages the tasks, task_queue and waker_cache
pub struct Executor {
	tasks: BTreeMap<TaskId, Task>,
	// reference counted ArrayQueue, shared between Executors and Wakers
	/// task_queue in a ArrayQueue avoids dynamic memory allocations, making it suitable for
	/// interrupt handlers
	/// To make it thread safe and multiple access .. we need Arc .. and to make it Arc
	/// compatible that is Atomic .. we need to wrap it inside a mutex so that each operation
	/// becomes atomic since you only release the lock when you're done
	task_queue: Arc<Mutex<BinaryHeap<Reverse<(u8, TaskId)>>>>,
	waker_cache: BTreeMap<TaskId, Waker>,
}

impl Executor {
	pub fn new() -> Self {
		Executor {
			tasks: BTreeMap::new(),
			// using a fixed queue, since interrupt handlers should not allocate on push
			// here we pass the size of the ArrayQueue .. hence no dynamic allocations
			task_queue: Arc::new(Mutex::new(BinaryHeap::new())),
			waker_cache: BTreeMap::new(),
		}
	}

	/// adds a new task to the executor
	/// panics if the queue is already full
	pub fn spawn(
		&mut self,
		task: Task,
	) {
		let task_id = task.id;
		let task_priority = task.meta.priority;
		if self.tasks.insert(task.id, task).is_some() {
			panic!("task with same ID already in tasks");
		}
		let mut queue = self.task_queue.lock();
		queue.push(Reverse((task_priority, task_id)));
	}

	/// continuously runs tasks in the queue
	pub fn run(&mut self) -> ! {
		loop {
			self.run_ready_tasks();
		}
	}

	/// age the tasks by 1
	/// This one prevents starvation
	pub fn age_priorities(&mut self) {
		for task in self.tasks.values_mut() {
			if task.meta.dyn_priority < 255 {
				task.meta.dyn_priority += 1; // so here we age the task priority
				// the more it ages .. the more the priority
			}
		}
	}

	/// track execution time of the tasks
	pub fn track_execution_time(
		&mut self,
		task_id: TaskId,
	) {
		let task = self.tasks.get_mut(&task_id).unwrap();

		if task.meta.dyn_priority > 0 {
			task.meta.dyn_priority -= 1;
		}
	}

	/// To execute all tasks in the task_queue
	///
	/// Loop over all tasks in the task_queue, create a waker for each task and then poll them
	fn run_ready_tasks(&mut self) {
		// increase the age of the remaining tasks -- increase the priority
		self.age_priorities();

		// destructure 'self' to avoid borrow checker errors
		let Self { tasks, task_queue, waker_cache } = self;

		// retrieves the tasks, creates a waker, and polls the task
		while let Some(Reverse((_, task_id))) = task_queue.lock().pop() {
			let mut poll_result = {
				let task = match tasks.get_mut(&task_id) {
					Some(task) => task,
					None => continue,
				};
				let task_meta_data: TaskMetadata = TaskMetadata {
					priority: task.meta.priority,
					dyn_priority: 0, // TODO -- what should it be?
				};
				let waker = waker_cache
					.entry(task_id)
					.or_insert_with(|| TaskWaker::new(task_id, task_meta_data, task_queue.clone()));

				let mut context = Context::from_waker(waker);

				task.poll(&mut context)
			};

			match poll_result {
				Poll::Ready(()) => {
					// task done -> remove it and its cached waker
					tasks.remove(&task_id);
					waker_cache.remove(&task_id);
				},
				Poll::Pending => {
					let dyn_p = tasks[&task_id].meta.dyn_priority;
					self.track_execution_time(task_id); // track the execution time
					task_queue.lock().push(Reverse((dyn_p, task_id)));
				},
			}
		}
	}

	/// save power when no tasks are available
	/// halts CPU if empty
	/// CPU put to sleep
	fn sleep_if_idle(&self) {
		use x86_64::instructions::interrupts::{self, enable_and_hlt};

		interrupts::disable();

		if self.task_queue.lock().is_empty() {
			enable_and_hlt();
		} else {
			interrupts::enable();
		}
	}
}

/// Custom waker for our executor
struct TaskWaker {
	task_id: TaskId,
	meta: TaskMetadata,
	task_queue: Arc<Mutex<BinaryHeap<Reverse<(u8, TaskId)>>>>,
}

impl TaskWaker {
	fn new(
		task_id: TaskId,
		meta: TaskMetadata,
		task_queue: Arc<Mutex<BinaryHeap<Reverse<(u8, TaskId)>>>>,
	) -> Waker {
		Waker::from(Arc::new(TaskWaker { task_id, meta, task_queue }))
	}

	fn wake_task(&self) {
		let priority = self.task_queue.lock().push(Reverse((self.meta.priority, self.task_id)));
	}
}

use alloc::task::Wake;
use spin::Mutex;
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