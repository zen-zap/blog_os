//! in src/task/pinh.rs
//! contains the functions for priority inheritance

// for priority inheritance, we would need to maintain a list of tree of tasks that depend on
// other tasks, or require some resource that another task is using. We might also need a
// dependency tree to manage the dependencies between tasks -- this doesn't really come under
// priority inheritance I guess

use crate::task::{Task, TaskId};
use alloc::sync::Arc;
use alloc::vec::Vec;

/// Represents any kind of resource
/// TODO -- extend this, maybe we could change it to something like file descriptors
/// but not everything would be represented with file descriptors I think
#[derive(Clone)]
pub struct Resource;

/// Represents a lock on a resource
/// TODO -- remove the wrapping of Option around the Resource, a lock should always be on a resource
///
/// It tracks the owner of the resource and the tasks waiting on it
/// Handles priority propagation
#[derive(Clone)]
pub struct PriLock {
	resource: Option<Resource>,
	owner: Option<TaskId>,
	waiters: Vec<TaskId>,
}

// We also need something to know which resource it is holding right? ..

impl PriLock {
	// TODO -- pass the resource here once priority inheritance is ready
	pub fn new(owner: TaskId) -> PriLock {
		PriLock { resource: None, owner: Some(owner), waiters: Vec::new() }
	}

	pub fn set_owner(
		&mut self,
		new_owner: TaskId,
	) {
		self.owner = Some(new_owner);
	}

	pub fn add_waiter(
		&mut self,
		waiter: TaskId,
	) {
		self.waiters.push(waiter);
	}

	/// Method to boost the priority of the owner if there are tasks with higher priority waiting
	/// for the owner
	pub fn propagate_priority(
		&mut self,
		tasks: &mut Vec<Task>,
		waiter_id: TaskId,
	) {
		if let Some(owner_id) = self.owner {
			let waiter_task_priority =
				tasks.iter().find(|t| t.id == waiter_id).unwrap().meta.dyn_priority;
			let owner_task = tasks.iter_mut().find(|t| t.id == owner_id).unwrap();

			// if the waiter has a higher priority, we boost the owners priority
			// then propagate it to the other locks the owner depends on
			if waiter_task_priority > owner_task.meta.dyn_priority {
				owner_task.meta.dyn_priority = waiter_task_priority;

				let locks_held = owner_task.meta.locks_held.clone();
				for mut lock in locks_held {
					lock.propagate_priority(tasks, owner_id);
				}
			}
		}
	}

	/// try to acquire the lock of a resource that is PriLock
	/// If the lock is not owned by anyone .. it gets owned by the passed TaskId
	/// else the passed TaskId gets added to the waiters of the specified lock.
	pub fn lock_acquire(
		&mut self,
		tasks: &mut Vec<Task>,
		task_id: TaskId,
	) {
		if self.owner.is_none() {
			self.owner = Some(task_id);
		} else {
			self.add_waiter(task_id);
			self.propagate_priority(tasks, task_id);
		}
	}

	pub fn lock_release(
		&mut self,
		tasks: &mut Vec<Task>,
	) {
		if let Some(owner_id) = self.owner {
			let owner_task = tasks.iter_mut().find(|t| t.id == owner_id).unwrap();

			owner_task.meta.dyn_priority = owner_task.meta.base_priority;

			if let Some(waiter_id) = self.waiters.pop() {
				self.owner = Some(waiter_id);
				self.propagate_priority(tasks, waiter_id);
			} else {
				self.owner = None;
			}
		}
	}
}
