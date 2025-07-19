//! in src/task/executor.rs
//!
//! The Executor is the Scheduler's Core

use crate::task::{Task, TaskId, TaskMetadata, lock::LockId, pinh::PriLock};
use alloc::{
	collections::{BTreeMap, BinaryHeap},
	sync::Arc,
	task::Wake,
	vec::Vec,
};
use core::{
	cmp::Reverse,
	sync,
	task::{Context, Poll, Waker},
};
use crossbeam_queue::ArrayQueue;
use futures_util::task::waker;
use spin::Mutex;
use x86_64::instructions::hlt;

/// Manages the tasks, task_queue and waker_cache
///
/// This will be only owner of all the PriLocks.
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

impl Executor {
	/// Creates a new executor
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
		// IMPORTANT: do we need to add them to the queue here?
		// They should be up for execution when they have the lock on their required resource
	}

	/// continuously runs tasks in the queue
	///
	/// It never returns
	pub fn run(&mut self) -> ! {
		loop {
			if self.task_queue.lock().is_empty() {
				self.sleep_if_idle();
			} else {
				self.run_ready_tasks();
			}
		}
	}

	/// Creates a new Lock and returns its ID
	pub fn create_lock(&mut self) -> LockId {
		// make a new ID
		let lock_id = LockId::new();
		// properly define it: ID and Lock
		// and store it
		self.locks.insert(lock_id, PriLock::new());
		lock_id
	}

	/// A task requires a lock
	pub fn acquire_lock(
		&mut self,
		task_id: TaskId,
		lock_id: LockId,
	) {
		let waiter_priority = self.tasks[&task_id].meta.dyn_priority;
		// get the lock first
		let lock = self.locks.get_mut(&lock_id).expect("Invalid LockId");

		if let Some(owner_id) = lock.owner {
			// LOCK is ALREADY HELD
			if owner_id != task_id {
				// check if the owner is the requester, if not then proceed
				// the current task goes to the waiting queue of the lock
				lock.waiters.push(task_id);

				// NOTE: Implemented Priority Inheritance here
				// We would need to compare the priority of the added task and the owner
				// Increase owner priority if it has less
				let lock_owner = self.tasks.get_mut(&owner_id).unwrap();
				// get the larger priority of the 2
				let owner_new_priority = lock_owner.meta.dyn_priority.max(waiter_priority);
				// The owner is probably in the task_queue, since it is an owner
				// we need to update it with the new priority
				// just re-add it ig? the binary heap can handle duplicates
				lock_owner.meta.dyn_priority = owner_new_priority;
				self.task_queue.lock().push(Reverse((owner_new_priority, owner_id)));

				// IMPORTANT: This waiter task is now blocked. Not added to the queue!
				// It could be unblocked by `release_lock`
			}
		} else {
			// LOCK is FREE
			// make the requesting task the owner of the lock
			lock.owner = Some(task_id);
			let task = self.tasks.get_mut(&task_id).unwrap();
			task.meta.locks_held.push(lock_id);

			// The task can continue running, add it to the queue
			let dyn_p = task.meta.dyn_priority;
			self.task_queue.lock().push(Reverse((dyn_p, task_id)));
		}
	}

	pub fn release_lock(
		&mut self,
		task_id: TaskId,
		lock_id: LockId,
	) {
		// immutable borrow here
		let lock = self.locks.get(&lock_id).unwrap();
		if lock.owner != Some(task_id) {
			panic!("Task {:?} tried to release a lock it doesn't own", task_id);
		}

		let releasing_task_meta = &self.tasks[&task_id].meta;
		let mut new_priority = releasing_task_meta.base_priority;

		for other_lock_id in &releasing_task_meta.locks_held {
			// skip the lock we're releasing
			if *other_lock_id == lock_id {
				continue;
			}

			let other_lock = &self.locks[other_lock_id];

			if let Some(highest_waiter_id) = other_lock
				.waiters
				.iter()
				.max_by_key(|waiter_id| self.tasks[waiter_id].meta.dyn_priority)
			{
				let highest_waiter_priority = self.tasks[highest_waiter_id].meta.dyn_priority;
				if highest_waiter_priority > new_priority {
					new_priority = highest_waiter_priority;
				}
			}
		}

		let releasing_task = self.tasks.get_mut(&task_id).unwrap();
		releasing_task.meta.dyn_priority = new_priority;
		releasing_task.meta.locks_held.retain(|x| *x != lock_id);

		let lock = self.locks.get_mut(&lock_id).unwrap(); // mutable re-borrow
		// After removing the current task, we add the new task to the task_queue
		if let Some(waiter_id) = lock.waiters.pop() {
			lock.owner = Some(waiter_id);

			let new_owner_task = self.tasks.get_mut(&waiter_id).unwrap();
			new_owner_task.meta.locks_held.push(lock_id);

			// wake up this task by adding it to the task_queue
			let dyn_p = new_owner_task.meta.dyn_priority;
			// add the waiter to the queue
			self.task_queue.lock().push(Reverse((dyn_p, waiter_id)));
		} else {
			// No tasks waiting on this resource lock
			lock.owner = None;
		}
	}

	/// bump task priorities by 1
	/// This one prevents starvation
	fn age_priorities(&mut self) {
		for task in self.tasks.values_mut() {
			if task.meta.dyn_priority < 255 {
				task.meta.dyn_priority += 1; // so here we age the task priority
				// the more it ages .. the more the priority
			}
		}
	}

	/// decrease the priority of a task if it keeps running for too long
	///
	/// Prevents a single task from hogging the CPU
	fn demote_task(
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
		self.age_priorities();
		// destructure to use shorter names
		// retrieves the tasks, creates a waker, and polls the task
		let task_id_popped = { self.task_queue.lock().pop() };

		if let Some(Reverse((_, task_id))) = task_id_popped {
			let mut poll_result = {
				// grab the task
				let task = self.tasks.get(&task_id).unwrap();
				// get or make its waker
				let task_queue = self.task_queue.clone();
				let waker = self
					.waker_cache
					.entry(task_id)
					.or_insert_with(|| TaskWaker::new(task_id, task_queue));
				// construct the context from the waker, you poll on the Context,
				// it provides the waker to the task along with other relevant information
				let mut context = Context::from_waker(waker);
				// poll the Context
				self.tasks.get_mut(&task_id).unwrap().poll(&mut context)
			};

			match poll_result {
				Poll::Ready(()) => {
					// NOTE: Make sure no lock is orphaned
					// task done -> remove it, locks held by it and its cached waker
					// since the task is done, just grab it entirely
					// IMPORTANT: is this a good solution to this?
					let locks_held_by_task =
						core::mem::take(&mut self.tasks.get_mut(&task_id).unwrap().meta.locks_held);
					// we no longer hold an immutable borrow of task.meta
					for lock in locks_held_by_task {
						self.release_lock(task_id, lock);
						// Simply using this defied borrow-checker
					}
					self.tasks.remove(&task_id);
					self.waker_cache.remove(&task_id);
				},
				Poll::Pending => {
					// Task is not finished, it will be re-queued by the waker if its needed
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
	task_queue: Arc<Mutex<BinaryHeap<Reverse<(u8, TaskId)>>>>,
}

impl TaskWaker {
	fn new(
		task_id: TaskId,
		task_queue: Arc<Mutex<BinaryHeap<Reverse<(u8, TaskId)>>>>,
	) -> Waker {
		let w = TaskWaker {
			task_id,
			task_queue,
			// store a raw pointer; safe as long as executor outlives this waker
		};
		unsafe { Waker::from(Arc::new(w)) }
	}

	/// Puts the tasks ID back into the task_queue
	/// the Executor determines its priority when popped
	fn wake_task(&self) {
		// the executor will handle this when it runs this task
		// NOTE: The priority here isn't used, the executor re-evaluates
		self.task_queue.lock().push(Reverse((0, self.task_id)));
	}
}

impl Wake for TaskWaker {
	fn wake(self: Arc<Self>) {
		self.wake_task();
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
