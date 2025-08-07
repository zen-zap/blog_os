//! in src/fs/simple_fs.rs

use crate::fs::block_handler::*;
use crate::fs::disk_handler::*;
use alloc::{string::String, vec::Vec};

#[derive(Debug, Copy, Clone)]
pub struct FileHandler(pub usize);

#[derive(Debug)]
pub enum FileError {
	FileNotFound,
	FileExists,
	NoSpace,
	InvalidHandle,
}

pub trait FileSystem {
	fn create_file(
		&mut self,
		name: &str,
	) -> Result<FileHandler, FileError>;
	fn delete_file(
		&mut self,
		name: &str,
	) -> Result<(), FileError>;
	fn open_file(
		&mut self,
		name: &str,
	) -> Result<FileHandler, FileError>;
	fn list_file(&mut self) -> Result<Vec<String>, FileError>;
}

/// SFS - Simple File System
pub struct SFS {
	disk_image: DiskImage,
	sb: SuperBlock,
}

// It should hold access to the disk image right?

impl FileSystem for SFS {
	fn create_file(
		&mut self,
		name: &str,
	) -> Result<FileHandler, FileError> {
		todo!()
	}

	fn delete_file(
		&mut self,
		name: &str,
	) -> Result<(), FileError> {
		todo!()
	}

	fn open_file(
		&mut self,
		name: &str,
	) -> Result<FileHandler, FileError> {
		todo!()
	}

	fn list_file(&mut self) -> Result<Vec<String>, FileError> {
		todo!()
	}
}
