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
#[derive(Clone, Default)]
pub struct PriLock {
	pub resource: Option<Resource>,
	pub owner: Option<TaskId>,
	pub waiters: Vec<TaskId>,
}

// We also need something to know which resource it is holding right? ..

impl PriLock {
	pub fn new() -> Self {
		Default::default() // new lock with no owners and no waiters
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

	/*/// Method to boost the priority of the owner if there are tasks with higher priority waiting
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
	}*/
}
