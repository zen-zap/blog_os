//! in src/task/executor.rs
//!
//! The Executor is the Scheduler Core

use super::{PriLock, Task, TaskId, TaskMetadata};
use alloc::{collections::BTreeMap, collections::BinaryHeap, sync::Arc};
use core::cmp::Reverse;
use core::task::{Context, Poll, Waker};
use crossbeam_queue::ArrayQueue;
use futures_util::task::waker;

/// Manages the tasks, task_queue and waker_cache
///
/// The Executor manages all tasks and locks in the system
/// Handles scheduling logic and priority inheritance logic
/// Provides global access to locks and ensures consistency across tasks
pub struct Executor {
	/// list of all tasks
	tasks: BTreeMap<TaskId, Task>,
	// reference counted ArrayQueue, shared between Executors and Wakers
	/// task_queue in a ArrayQueue avoids dynamic memory allocations, making it suitable for
	/// interrupt handlers
	/// To make it thread safe and multiple access .. we need Arc .. and to make it Arc
	/// compatible that is Atomic .. we need to wrap it inside a mutex so that each operation
	/// becomes atomic since you only release the lock when you're done
	task_queue: Arc<Mutex<BinaryHeap<Reverse<(u8, TaskId)>>>>,
	waker_cache: BTreeMap<TaskId, Waker>,
	/// Global table of all the locks
	locks: BTreeMap<LockId, PriLock>,
}

pub struct LockId(u8);

impl Executor {
	pub fn new() -> Self {
		Executor {
			tasks: BTreeMap::new(),
			// using a fixed queue, since interrupt handlers should not allocate on push
			// here we pass the size of the ArrayQueue .. hence no dynamic allocations
			task_queue: Arc::new(Mutex::new(BinaryHeap::new())),
			waker_cache: BTreeMap::new(),
			locks: BTreeMap::new(),
		}
	}

	/// adds a new task to the executor
	/// panics if the queue is already full
	pub fn spawn(
		&mut self,
		task: Task,
	) {
		let task_id = task.id;
		let task_priority = task.meta.base_priority;
		if self.tasks.insert(task.id, task).is_some() {
			panic!("task with same ID already in tasks");
		}
		let mut queue = self.task_queue.lock();
		queue.push(Reverse((task_priority, task_id)));
	}

	/// continuously runs tasks in the queue
	///
	/// It never returns
	pub fn run(&mut self) -> ! {
		loop {
			self.run_ready_tasks();
		}
	}

	/// age the tasks by 1
	/// This one prevents starvation
	fn age_priorities(&mut self) {
		for task in self.tasks.values_mut() {
			if task.meta.dyn_priority < 255 {
				task.meta.dyn_priority += 1; // so here we age the task priority
				// the more it ages .. the more the priority
			}
		}
	}

	/// track execution time of the tasks
	fn track_execution_time(
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
	/// This is kinda the heartbeat ig
	///
	/// Loop over all tasks in the task_queue, create a waker for each task and then poll them
	fn run_ready_tasks(&mut self) {
		// increase the age of the remaining tasks in the Executor -- increase the priority
		// bumps priority by 1
		self.age_priorities();

		// destructure to use shorter names
		let Self { tasks, task_queue, waker_cache, locks } = self;

		// retrieves the tasks, creates a waker, and polls the task
		while let Some(Reverse((_, task_id))) = task_queue.lock().pop() {
			let mut poll_result = {
				// grab the task
				let task = match tasks.get_mut(&task_id) {
					Some(task) => task,
					None => continue,
				};
				// get the meta data from the task
				let task_meta_data: TaskMetadata = TaskMetadata {
					base_priority: task.meta.base_priority,
					dyn_priority: 0, // TODO -- what should it be?
					locks_held: Vec::new(),
				};
				// get or make its waker
				let waker = waker_cache
					.entry(task_id)
					.or_insert_with(|| TaskWaker::new(task_id, task_meta_data, task_queue.clone()));
				// construct the context from the waker, you poll on the Context
				// Context provides the waker to the task along with other relevant information
				let mut context = Context::from_waker(waker);
				// poll the Context
				task.poll(&mut context)
			};

			match poll_result {
				Poll::Ready(()) => {
					// task done -> remove it and its cached waker
					tasks.remove(&task_id);
					waker_cache.remove(&task_id);
				},
				Poll::Pending => {
					// if not done, re-queue with its current dynamic priority
					let dyn_p = tasks[&task_id].meta.dyn_priority;
					//track_execution_time(task_id); // track the execution time TODO
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

	/// Calls acquire_lock when a task requests a lock
	fn acquire_lock(
		&mut self,
		lock_id: LockId,
		task_id: TaskId,
	) {
	}

	fn release_lock(
		&mut self,
		lock_id: LockId,
		task_id: TaskId,
	) {
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
		let priority = self.task_queue.lock().push(Reverse((self.meta.dyn_priority, self.task_id)));
	}
}

use alloc::task::Wake;
use alloc::vec::Vec;
use spin::Mutex;
use x86_64::instructions::hlt;

// gotta convert our TaskWaker to a Waker instance first
// could also be done by using RawWaker
impl Wake for TaskWaker {
	fn wake(self: Arc<Self>) {
		self.task_queue.lock().push(Reverse((self.meta.dyn_priority, self.task_id)));
	}

	fn wake_by_ref(self: &Arc<Self>) {
		self.wake_task();
	}
}

/*
RTOS: ------
Real-time operating systems (RTOS) are ones that guarantees the latency (time delay) of interrupts.
In other words, the time delay between the instant when an interrupt occurs
and the time when a special interrupt service routine is activated to service that interrupt is bounded.

An RTOS will be most useful when multiple interrupts may occur at unspecified times.
Real-time operating systems are more popular with general-purpose processors.

Critical Region: ------
A critical region is a delimited section of code where access to a protected variable is allowed,
typically controlled by semaphores or Boolean conditions to prevent concurrent access by multiple threads.
The piece of code that access a shared resource is called the critical section.

What is Priority Inversion? ----- RTOS bug

A high priority task gets delayed due to low priority task holding the lock to a shared resource.
Consider this scenario, there are 3 processes:
P1 -> highest priority
P3 -> lowest priority
P2 -> priority between P1 and P3

P3 becomes ready and enters its critical region, getting the lock on a shared resource.
P2 becomes ready and preempts P3
P1 becomes ready and preempts P2 and starts to run. It continues till it reaches its critical
region. P1 stops when its reaches its critical section because P3 was preempted before it could
release the lock on the shared resource. P1 can continue only when P3 has completed its execution
or at least till it has released the lock on the shared resource.

One method for dealing with this is Priority Inheritance ----
The most common method for dealing with priority inversion is priority inheritance,
which promotes the priority of any process when it requests a resource from the operating system.
The priority of the process temporarily becomes higher than that of any other process that may use the resource.
This ensures that the process will continue executing once it has the resource so that it can finish its work with the resource,
return it to the operating system, and allow other processes to use it.
Once the process is finished with the resource, its priority is demoted to its normal value.

AutoBoost -- Used in windows, to raise the priority of the owner of the lock to the highest priority,
 so it gets to complete its execution

There is also something called Priority Ceiling Protocol -----
each resource has a “ceiling” priority;
tasks must raise themselves to that ceiling when entering the section

Semaphores ----
This is a synchronization primitive that can be used to control access to a shared resource by
multiple threads or tasks. It maintains a counter representing the number of available "permits"
for accessing the resource.

Each acquire or wait operation on the resource decrements the counter. If the counter is zero,
the caller blocks until a permit is released. Each release or signal increments the counter,
possibly waking a waiting thread.

Types of Semaphores:
-> Binary Semaphores: Simple Mutex Lock. 0 or 1. Used to implement the solution of critical
problems with multiple processes and a single resource
-> Counting Semaphores: To control access to a given resource consisting of a finite number of
instances. The semaphore is initialized to the number of resources available.

 */

/*
ASCII Flow Chart of the Executor ---- by ChatGPT


							 ┌────────────────────────┐
							 │        Executor        │
							 └──────────┬─────────────┘
										│
						   ┌────────────▼────────────┐
						   │   age_priorities()      │
						   └────────────┬────────────┘
										│
				  ┌─────────────────────▼─────────────────────┐
				  │ pop highest (dyn_priority, TaskId) from   │
				  │     task_queue (BinaryHeap)               │
				  └─────────────────────┬─────────────────────┘
										│
						 ┌──────────────▼──────────────┐
						 │ lookup Task in tasks map    │
						 └──────────────┬──────────────┘
										│
					   ┌────────────────▼────────────────┐
					   │ get-or-create Waker for TaskId  │
					   └────────────────┬────────────────┘
										│
						   ┌────────────▼────────────┐
						   │   poll(&mut Context)    │
						   └────────────┬────────────┘
										│
					┌───────────────────▼───────────────────┐
					│ is Poll::Ready or Poll::Pending?      │
					└────────────┬────────────────┬─────────┘
								 │                │
			 ┌───────────────────▼───┐       ┌────▼──────────┐
			 │ Poll::Ready(())       │       │ Poll::Pending │
			 └───────┬───────────────┘       └────────┬──────┘
					 │                                │
		 ┌───────────▼───────────┐       ┌────────────▼───────────┐
		 │ remove Task & Waker   │       │ track_execution_time() │
		 └───────────────────────┘       └───┬────────────────────┘
											 │
									┌────────▼─────────┐
									│ re-push (dyn_p,  │
									│   TaskId) onto   │
									│   task_queue     │
									└────────┬─────────┘
											 │
									┌────────▼─────────┐
									│ sleep_if_idle()? │
									└────────┬─────────┘
											 │
								 yes ◄───────┘   no
								 ┌──────────┐
								 │ halt CPU │
								 └──────────┘
 */
