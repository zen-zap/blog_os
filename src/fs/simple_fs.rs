//! in src/fs/simple_fs.rs

use super::{block_dev::BlockDevice, layout::*};
use crate::fs::layout::FileType::File;
use crate::println;
use alloc::{string::String, vec::Vec};
use core::convert::TryFrom;
use core::ptr::write;
use pc_keyboard::KeyCode::P;
use zerocopy::{FromBytes, IntoBytes, KnownLayout, U16, U32, U64};

const MAGIC_NUMBER: u32 = 0x_DEAD_BEEF;
const ROOT_DIRECTORY_INODE: u64 = 0;

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

		let inode_table_blocks = capacity / 10; // 10% of the total capacity goes to the INODE_TABLE
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

		let mut superblock_buffer = [0u8; BLOCK_SIZE];
		let dsb = DiskSuperBlock::from(sb);

		superblock_buffer[..size_of::<DiskSuperBlock>()].copy_from_slice(dsb.as_bytes());

		device
			.write_blocks(SUPERBLOCK_BLOCK, &superblock_buffer)
			.map_err(|_| FileSystemError::BlockError)?;

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

	/// Allocates a data block following a read-modify-write pattern
	pub fn allocate_data_block(&mut self) -> Result<u64, FileSystemError> {
		let mut bm_buffer = [0u8; BLOCK_SIZE];

		self.device
			.read_blocks(DATA_BITMAP_BLOCK, &mut bm_buffer)
			.map_err(|_| FileSystemError::BlockError)?;

		let mut data_bitmap = Bitmap::new(&mut bm_buffer);

		let free_idx = data_bitmap.find_and_set_first_free().ok_or(FileSystemError::NoSpace)?;

		self.device
			.write_blocks(DATA_BITMAP_BLOCK, &bm_buffer)
			.map_err(|_| FileSystemError::BlockError)?;

		let abs_block = self.superblock.data_block_start + free_idx as u64;

		Ok(abs_block)
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

	/// Adds a new directory entry into a directory block buffer at a given slot index
	///
	/// A block would have multiple directory entries, slot is the index of this.
	pub fn write_dirent_into_block(
		&self,
		block: &mut [u8; BLOCK_SIZE],
		slot: usize,
		inode: u64,
		name: &[u8],
	) -> Result<(), FileSystemError> {
		if name.len() > DIR_NAME_MAX {
			return Err(FileSystemError::NameTooLong);
		}

		let start = slot * DIR_ENTRY_SIZE;
		let end = start + DIR_ENTRY_SIZE;

		// Build an entry
		let mut entry_bytes = [0u8; DIR_ENTRY_SIZE];
		// Safe because DiskDirEntry is IntoBytes and exactly DIR_ENTRY_SIZE
		let mut entry = DiskDirEntry {
			inode: U64::new(inode),
			name_len: U16::new(name.len() as u16),
			flags: U16::new(DIRENT_USED),
			name: [08; DIR_NAME_MAX],
		};

		entry.name[..name.len()].copy_from_slice(name);
		// make the above entry into a buffer
		entry_bytes.copy_from_slice(entry.as_bytes());

		block[start..end].copy_from_slice(&entry_bytes);
		Ok(())
	}

	/// Find free slot in a directory block (first block only for now), returns slot index
	pub fn find_free_dir_slot(
		&self,
		block: &[u8; BLOCK_SIZE],
	) -> Option<usize> {
		for i in 0..DIR_ENTRIES_PER_BLOCK {
			let start = i * DIR_ENTRY_SIZE;
			let end = start + DIR_ENTRY_SIZE;

			if let Ok(entry) = DiskDirEntry::ref_from_bytes(&block[start..end]) {
				let used = (entry.flags.get() & DIRENT_USED) != 0;
				let inode = entry.inode.get();

				if !used || inode == 0 {
					return Some(i);
				}
			}
		}
		None
	}

	// Initialize Root Directory: Inode 0, allocate one data block
	pub fn init_root_directory(&mut self) -> Result<(), FileSystemError> {
		let mut ibuf = [0u8; BLOCK_SIZE];
		self.device
			.read_blocks(INODE_BITMAP_BLOCK, &mut ibuf)
			.map_err(|_| FileSystemError::BlockError)?;

		{
			let mut bm = Bitmap::new(&mut ibuf);
			if !bm.is_set(0) {
				bm.set(0);
			}
		}

		self.device
			.write_blocks(INODE_BITMAP_BLOCK, &ibuf)
			.map_err(|_| FileSystemError::BlockError)?;

		let data_block = self.allocate_data_block()?;

		let mut root = Inode {
			mode: FileType::Directory,
			user_id: 0,
			group_id: 0,
			link_count: 2, // "." and ".."
			size_in_bytes: 0,
			last_access_time: 0,
			last_modification_time: 0,
			creation_time: 0,
			direct_pointers: [0u64; 10],
			indirect_pointer: 0,
		};

		root.direct_pointers[0] = data_block;
		self.write_inode(root, 0)?;

		let mut dir_block = [0u8; BLOCK_SIZE];
		self.write_dirent_into_block(&mut dir_block, 0, 0, b".")?;
		self.write_dirent_into_block(&mut dir_block, 1, 0, b"..")?;

		self.device
			.write_blocks(data_block, &dir_block)
			.map_err(|_| FileSystemError::BlockError)?;

		Ok(())
	}

	pub fn add_root_dir_entry(
		&mut self,
		inode: u64,
		name: &str,
	) -> Result<(), FileSystemError> {
		if name.as_bytes().len() > DIR_NAME_MAX {
			return Err(FileSystemError::NameTooLong);
		}

		// Root is inode 0
		let root = self.read_inode(0)?;
		let block = root.direct_pointers[0];

		if block == 0 {
			return Err(FileSystemError::CorruptLayout);
		}

		let mut dir_block = [0u8; BLOCK_SIZE];
		self.device
			.read_blocks(block, &mut dir_block)
			.map_err(|_| FileSystemError::BlockError)?;

		let slot = self.find_free_dir_slot(&dir_block).ok_or(FileSystemError::NoSpace)?;

		self.write_dirent_into_block(&mut dir_block, slot, inode, name.as_bytes())?;

		self.device
			.write_blocks(block, &dir_block)
			.map_err(|_| FileSystemError::BlockError)?;

		Ok(())
	}

	fn create_file_in_root(
		&mut self,
		name: &str,
	) -> Result<(u64 /*inode index*/, u64 /*dir block*/), FileSystemError> {
		if name.as_bytes().len() > DIR_NAME_MAX || name.is_empty() {
			return Err(FileSystemError::NameTooLong);
		}

		// Read root directory block
		let root_dir_inode = self.read_inode(ROOT_DIRECTORY_INODE)?;
		if root_dir_inode.mode != FileType::Directory {
			return Err(FileSystemError::CorruptLayout);
		}

		let dir_block = root_dir_inode.direct_pointers[0];
		if dir_block == 0 {
			return Err(FileSystemError::CorruptLayout);
		}
		let mut dir_block_buf = [0u8; BLOCK_SIZE];
		self.device
			.read_blocks(dir_block, &mut dir_block_buf)
			.map_err(|_| FileSystemError::BlockError)?;

		// Collision check and find slot
		let mut empty_slot_index: Option<usize> = None;
		let entries = DirEntryBlock::new(&dir_block_buf);
		for (i, entry) in entries.enumerate() {
			let is_used = (entry.flags.get() & DIRENT_USED) != 0;
			if is_used {
				let entry_name_len = entry.name_len.get() as usize;
				if &entry.name[..entry_name_len] == name.as_bytes() {
					return Err(FileSystemError::CorruptLayout); // use FileError::FileExists at call site
				}
			} else if empty_slot_index.is_none() {
				empty_slot_index = Some(i);
			}
		}
		let slot_index = empty_slot_index.ok_or(FileSystemError::NoSpace)?;

		// Allocate inode and write it
		let inode_index = self.allocate_inode()?;
		let new_inode = Inode {
			mode: FileType::File,
			user_id: 0,
			group_id: 0,
			link_count: 1,
			size_in_bytes: 0,
			last_access_time: 0,
			last_modification_time: 0,
			creation_time: 0,
			direct_pointers: [0u64; 10],
			indirect_pointer: 0,
		};
		self.write_inode(new_inode, inode_index)?;

		// Write directory entry into buffer
		self.write_dirent_into_block(&mut dir_block_buf, slot_index, inode_index, name.as_bytes())?;

		// PERSIST THE UPDATED DIRECTORY BLOCK (this was missing)
		self.device
			.write_blocks(dir_block, &dir_block_buf)
			.map_err(|_| FileSystemError::BlockError)?;

		Ok((inode_index, dir_block))
	}
}

/// Holds the inode index of the file
#[derive(Debug, Copy, Clone)]
pub struct FileHandler(pub usize);

#[derive(Debug)]
pub enum FileError {
	BlockReadError,
	DirectoryFull,
	BlockWriteError,
	FileNotFound,
	FileExists,
	CreationFailed,
	NoSpace,
	InvalidHandle,
	InvalidName,
	Corrupt,
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
	NameTooLong,
	CorruptLayout,
	InvalidSuperBlock,
}

impl<D: BlockDevice> FileSystem for SFS<D> {
	fn create_file(
		&mut self,
		name: &str,
	) -> Result<FileHandler, FileError> {
		let (inode_index, _dir_block) = self.create_file_in_root(name).map_err(|e| match e {
			FileSystemError::NameTooLong => FileError::InvalidName,
			FileSystemError::NoSpace => FileError::NoSpace,
			FileSystemError::CorruptLayout => FileError::Corrupt,
			_ => FileError::CreationFailed,
		})?;
		println!("[FS] Created file '{}' with inode #{}", name, inode_index);
		Ok(FileHandler(inode_index as usize))
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
