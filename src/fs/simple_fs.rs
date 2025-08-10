//! in src/fs/simple_fs.rs

use super::{
	block_dev::{BlockDevice, BlockError},
	layout::*,
};
use crate::fs::layout::FileType::File;
use crate::println;
use alloc::{string::String, vec::Vec};
use core::convert::TryFrom;
use zerocopy::{FromBytes, IntoBytes};

const MAGIC_NUMBER: u32 = 0x_DEAD_BEEF;

// TODO: Write a Wrapper for the VirtIoBlkDevice --- currently just using the trait implementations

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
		};

		let dsb = DiskSuperBlock::from(sb);
		let mut block = [0u8; BLOCK_SIZE];
		let dsb_bytes = dsb.as_bytes();
		block[..dsb_bytes.len()].copy_from_slice(dsb_bytes);

		device
			.write_blocks(SUPERBLOCK_BLOCK, dsb.as_bytes())
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

		let size = size_of::<DiskSuperBlock>();
		let disk_superblock = DiskSuperBlock::ref_from_bytes(&buffer[..size])
			.map_err(|_| FileSystemError::InvalidSuperBlock)?;

		let superblock = SuperBlock::try_from(*disk_superblock)
			.map_err(|_| FileSystemError::InvalidSuperBlock)?;

		if superblock.magic_number != MAGIC_NUMBER {
			return Err(FileSystemError::InvalidSuperBlock);
		}

		Ok(Self { device, superblock })
	}

	pub fn allocate_inode(&mut self) -> Result<u64, FileSystemError> {
		let mut bitmap_buffer = [0u8; BLOCK_SIZE];

		self.device
			.read_blocks(INODE_BITMAP_BLOCK, &mut bitmap_buffer)
			.map_err(|_| FileSystemError::BlockError)?;

		// we gotta wrap the buffer around this to work on it as a Bitmap
		let mut inode_bitmap = Bitmap::new(&mut bitmap_buffer);

		let free_inode_index =
			inode_bitmap.find_and_set_first_free().ok_or(FileSystemError::NoSpace)?;

		// here we're working a reference of the bitmap_buffer -- so it is still valid and can be
		// passed as the buffer to the write_blocks

		// so the write_blocks of the BlockDevice should be able to overwrite the contents of the
		// block if any exists
		self.device
			.write_blocks(self.superblock.inode_bitmap_block, &bitmap_buffer)
			.map_err(|_| FileSystemError::BlockError)?;

		Ok(free_inode_index as u64)
	}

	pub fn read_inode(
		&mut self,
		inode_index: u64,
	) -> Result<Inode, FileSystemError> {
		let block_num =
			self.superblock.inode_table_start_block + (inode_index / INODES_PER_BLOCK as u64);

		let offset_in_block = (inode_index % INODES_PER_BLOCK as u64) as usize * INODE_SIZE;

		let mut buffer = [0u8; BLOCK_SIZE];
		self.device
			.read_blocks(block_num, &mut buffer)
			.map_err(|_| FileSystemError::BlockError)?;

		// so here we read the disk inode from the buffer
		let size = size_of::<DiskInode>();
		let disk_inode =
			DiskInode::ref_from_bytes(&buffer[offset_in_block..(offset_in_block + size)])
				.map_err(|_| FileSystemError::BlockError)?;

		let inode = Inode::try_from(*disk_inode).map_err(|_| FileSystemError::BlockError)?;

		Ok(inode)
	}

	pub fn write_inode(
		&mut self,
		inode: Inode,
		inode_idx: u64,
	) -> Result<(), FileSystemError> {
		// then we have to know which actual inode to write this into
		// the free_inode_idx is just the index of the bit in the inode_bitmap
		// so we gotta fetch the inode tables now, then index from those tables

		let block_num =
			self.superblock.inode_table_start_block + (inode_idx / INODES_PER_BLOCK as u64);

		let offset_in_block = (inode_idx % INODES_PER_BLOCK as u64) as usize * INODE_SIZE;

		let mut buffer = [0u8; BLOCK_SIZE];
		self.device
			.read_blocks(block_num, &mut buffer)
			.map_err(|_| FileSystemError::BlockError)?;

		// so here we read the disk inode from the buffer
		let disk_inode = DiskInode::from(inode);
		//let inode_m = Inode::try_from(disk_inode).unwrap();
		let size = size_of::<DiskInode>();
		let inode_slice = &mut buffer[offset_in_block..(offset_in_block + size)];
		inode_slice.copy_from_slice(disk_inode.as_bytes());

		self.device
			.write_blocks(block_num, &buffer)
			.map_err(|_| FileSystemError::BlockError)?;

		Ok(())
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
	NoSpace,
	CorruptLayout,
	InvalidSuperBlock,
}
