//! in kernel/src/shell_task.rs
//!
//! File System Commands (FSC)
//!
//! This module provides a stateless utility for executing standard file system
//! commands (like `ls`, `cat`, `rm`). It acts as a bridge between the user shell
//! and the underlying Simple File System (SFS).

use crate::fs::simple_fs::FileSystem;
use crate::{GLOBALFSType, error, info, println};
use alloc::vec::Vec;
use no_std_async::Mutex as AsyncMutex;
use spin::Mutex;

/// A stateless utility struct for executing file system commands.
#[derive(Default)]
pub struct Fsc;

impl Fsc {
	/// Creates a new instance of the File System Command utility.
	pub fn new() -> Self {
		Fsc
	}

	/// Lists the contents of a directory.
	///
	/// # Arguments
	/// * `path` - The directory path to list.
	/// * `fs_mutex` - A reference to the global file system mutex.
	pub async fn ls(
		&self,
		path: &str,
		fs_mutex: &AsyncMutex<GLOBALFSType>,
	) {
		if path.is_empty() {
			println!("Usage: ls <path>");
			return;
		}

		let mut fs = fs_mutex.lock().await;
		let result = fs.list_file(path);
		drop(fs);

		match result {
			Ok(files) => {
				for file in files {
					println!("{}", file);
				}
			},
			Err(e) => error!("ls failed: {:?}", e),
		}
	}

	/// Creates a new, empty file.
	///
	/// # Arguments
	/// * `path` - The path of the file to create.
	/// * `fs_mutex` - A reference to the global file system mutex.
	pub async fn touch(
		&self,
		path: &str,
		fs_mutex: &AsyncMutex<GLOBALFSType>,
	) {
		if path.is_empty() {
			println!("Usage: touch <path>");
			return;
		}

		let mut fs = fs_mutex.lock().await;
		let result = fs.create_file(path);
		drop(fs);

		match result {
			Ok(_) => info!("Created '{}'", path),
			Err(e) => error!("touch failed: {:?}", e),
		}
	}

	/// Creates a new directory.
	///
	/// # Arguments
	/// * `path` - The path of the directory to create.
	/// * `fs_mutex` - A reference to the global file system mutex.
	pub async fn mkdir(
		&self,
		path: &str,
		fs_mutex: &AsyncMutex<GLOBALFSType>,
	) {
		if path.is_empty() {
			println!("Usage: mkdir <path>");
			return;
		}

		let mut fs = fs_mutex.lock().await;
		let result = fs.create_directory(path);
		drop(fs);

		match result {
			Ok(_) => info!("Created directory '{}'", path),
			Err(e) => error!("mkdir failed: {:?}", e),
		}
	}

	/// Deletes a file from the file system.
	///
	/// # Arguments
	/// * `path` - The path of the file to delete.
	/// * `fs_mutex` - A reference to the global file system mutex.
	pub async fn rm(
		&self,
		path: &str,
		fs_mutex: &AsyncMutex<GLOBALFSType>,
	) {
		if path.is_empty() {
			println!("Usage: rm <path>");
			return;
		}

		let mut fs = fs_mutex.lock().await;
		let result = fs.delete_file(path);
		drop(fs);

		match result {
			Ok(_) => info!("Deleted '{}'", path),
			Err(e) => error!("rm failed: {:?}", e),
		}
	}

	/// Reads and prints the contents of a file.
	///
	/// # Arguments
	/// * `path` - The path of the file to read.
	/// * `fs_mutex` - A reference to the global file system mutex.
	pub async fn cat(
		&self,
		path: &str,
		fs_mutex: &AsyncMutex<GLOBALFSType>,
	) {
		if path.is_empty() {
			println!("Usage: cat <path>");
			return;
		}

		let mut fs = fs_mutex.lock().await;
		let handle_result = fs.open_file(path);

		match handle_result {
			Ok(handle) => {
				let read_result = fs.read_file(handle);
				drop(fs); // Lock released; safe to print large text chunks

				match read_result {
					Ok(content) => println!("{}", content),
					Err(e) => error!("cat: read failed: {:?}", e),
				}
			},
			Err(e) => {
				drop(fs);
				error!("cat: open failed: {:?}", e);
			},
		}
	}

	/// Writes lines of text to a file, creating it if it does not exist.
	///
	/// # Arguments
	/// * `path` - The path of the file to write to.
	/// * `lines` - A slice containing string references representing lines of text.
	/// * `fs_mutex` - A reference to the global file system mutex.
	pub async fn write(
		&self,
		path: &str,
		lines: &[&str],
		fs_mutex: &AsyncMutex<GLOBALFSType>,
	) {
		if path.is_empty() {
			println!("Usage: write <path> [content]");
			return;
		}

		let mut fs = fs_mutex.lock().await;

		let handle = match fs.open_file(path) {
			Ok(h) => h,
			Err(_) => match fs.create_file(path) {
				Ok(h) => h,
				Err(e) => {
					drop(fs);
					error!("write: could not create file: {:?}", e);
					return;
				},
			},
		};

		let write_result = fs.write_file_lines(handle, lines);
		drop(fs);

		match write_result {
			Ok(_) => info!("Wrote to '{}'", path),
			Err(e) => error!("write failed: {:?}", e),
		}
	}
}
