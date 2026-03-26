use crate::{println, print, task::keyboard, error, info, GLOBALFSType};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, KeyCode, ScancodeSet1};
use spin::Mutex;
// to call the methods we need to import this
use crate::fs::simple_fs::FileSystem;

/// Struct to manage all the file system commands
pub struct Fsc;

impl Fsc {
	pub fn new() -> Self {
		Fsc
	}
	pub fn ls(&mut self, path: &str, fs_mutex: &Mutex<GLOBALFSType>) {
		if path.is_empty() {
			println!("Usage: ls <path>");
			return;
		}
		let mut fs = fs_mutex.lock();
		match fs.list_file(path) {
			Ok(files) => {
				for file in files {
					println!("{}", file);
				}
			}
			Err(e) => error!("ls failed: {:?}", e),
		}
	}

	pub fn touch(&mut self, path: &str, fs_mutex: &Mutex<GLOBALFSType>) {
		if path.is_empty() {
			println!("Usage: touch <path>");
			return;
		}
		let mut fs = fs_mutex.lock();
		match fs.create_file(path) {
			Ok(_) => info!("Created '{}'", path),
			Err(e) => error!("touch failed: {:?}", e),
		}

		// the lock is hopefully dropped here
	}

	pub fn mkdir(&mut self, path: &str, fs_mutex: &Mutex<GLOBALFSType>) {
		if path.is_empty() {
			println!("Usage: mkdir <path>");
			return;
		}
		let mut fs = fs_mutex.lock();
		match fs.create_directory(path) {
			Ok(_) => info!("Created directory '{}'", path),
			Err(e) => error!("mkdir failed: {:?}", e),
		}
	}

	pub fn rm(&mut self, path: &str, fs_mutex: &Mutex<GLOBALFSType>) {
		if path.is_empty() {
			println!("Usage: rm <path>");
			return;
		}
		let mut fs = fs_mutex.lock();
		match fs.delete_file(path) {
			Ok(_) => info!("Deleted '{}'", path),
			Err(e) => error!("rm failed: {:?}", e),
		}
	}

	pub fn cat(&mut self, path: &str, fs_mutex: &Mutex<GLOBALFSType>) {
		if path.is_empty() {
			println!("Usage: cat <path>");
			return;
		}
		let mut fs = fs_mutex.lock();
		match fs.open_file(path) {
			Ok(handle) => {
				match fs.read_file(handle) {
					Ok(content) => println!("{}", content),
					Err(e) => error!("cat: read failed: {:?}", e),
				}
			}
			Err(e) => error!("cat: open failed: {:?}", e),
		}
	}
	pub fn write(&mut self, path: &str, lines: &Vec<&str>, fs_mutex:
	&Mutex<GLOBALFSType>) {
		if path.is_empty() {
			println!("Usage: write <path> [content]");
			return;
		}
		let mut fs = fs_mutex.lock();
		let handle = match fs.open_file(path) {
			Ok(h) => h,
			Err(_) => {
				match fs.create_file(path) {
					Ok(h) => h,
					Err(e) => {
						error!("write: could not create file: {:?}", e);
						return;
					}
				}
			}
		};

		match fs.write_file_lines(handle, &lines) {
			Ok(_) => info!("Wrote to '{}'", path),
			Err(e) => error!("write failed: {:?}", e),
		}
	}
}