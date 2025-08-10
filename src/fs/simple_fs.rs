//! in src/fs/simple_fs.rs

use super::{
	block_dev::{BlockDevice, BlockError},
	layout::{
		BLOCK_SIZE, Bitmap, DATA_BITMAP_BLOCK, INODE_BITMAP_BLOCK, INODE_SIZE,
		INODE_TABLE_START_BLOCK, INODES_PER_BLOCK, SUPERBLOCK_BLOCK, SuperBlock,
	},
};
use crate::println;
use alloc::{string::String, vec::Vec};
use zerocopy::{FromBytes, IntoBytes};

const MAGIC_NUMBER: u32 = 0x_DEAD_BEEF;

/// SFS - Simple File System
#[derive(Debug)]
#[repr(C)]
pub struct SFS<D: BlockDevice> {
	device: D,
	superblock: SuperBlock,
}

impl<D: BlockDevice> SFS<D> {
	/// writes the superblock in the block device at block_id: 0
	pub fn format(mut device: D) -> Result<Self, FileSystemError> {
		println!("[FS] Formatting Device");

		let capacity: u64 = device.capacity() as u64;

		let inode_table_blocks = capacity / 5; // 5% of the total capacity goes to the INODE_TABLE
		let inode_count = inode_table_blocks * INODES_PER_BLOCK as u64;

		let data_block_start = INODE_TABLE_START_BLOCK + inode_table_blocks;
		let data_block_count = capacity - data_block_start; // this works … think about it

		let sb = SuperBlock {
			magic_number: MAGIC_NUMBER,
			total_blocks: capacity,
			inode_bitmap_block: INODE_BITMAP_BLOCK,
			data_bitmap_block: DATA_BITMAP_BLOCK,
			inode_table_start_block: INODE_TABLE_START_BLOCK,
			inode_count,
			data_block_start,
			data_block_count,
			_pad0: 0, // passing field -- required by IntoBytes trait
		};

		device
			.write_blocks(SUPERBLOCK_BLOCK, sb.as_bytes())
			.map_err(|_| FileSystemError::BlockError);

		let empty_bitmap_block = [0u8; BLOCK_SIZE];
		// Writing the INODE BITMAP BLOCK
		device
			.write_blocks(INODE_BITMAP_BLOCK, empty_bitmap_block.as_bytes())
			.map_err(|_| FileSystemError::BlockError)?;
		// Writing the DATA BITMAP BLOCK
		device
			.write_blocks(DATA_BITMAP_BLOCK, empty_bitmap_block.as_bytes())
			.map_err(|_| FileSystemError::BlockError)?;

		Ok(Self { device, superblock: sb })
	}

	/// Mounts an existing file system from a block device
	pub fn mount(mut device: D) -> Result<Self, FileSystemError> {
		let mut buffer = [0u8; BLOCK_SIZE];

		device
			.read_blocks(SUPERBLOCK_BLOCK, &mut buffer)
			.map_err(|_| FileSystemError::InvalidSuperBlock);

		let superblock = SuperBlock::read_from_bytes(&buffer[..])
			.map_err(|_| FileSystemError::InvalidSuperBlock)?;

		if superblock.magic_number != MAGIC_NUMBER {
			return Err(FileSystemError::InvalidSuperBlock);
		}

		Ok(Self { device, superblock })
	}
}

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

#[derive(Debug)]
pub enum FileSystemError {
	FormatFailed,
	MountFailed,
	BlockError,
	CorruptLayout,
	InvalidSuperBlock,
}
