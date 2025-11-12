//! in src/fs/simple_fs.rs

use super::{block_dev::BlockDevice, layout::*};
use crate::fs::layout::FileType::File;
use crate::println;
use alloc::string::ToString;
use alloc::{string::String, vec::Vec};
use core::convert::TryFrom;
use core::ptr::write;
use pc_keyboard::KeyCode::P;
use virtio_drivers::device::blk::VirtIOBlk;
use virtio_drivers::transport::pci::PciTransport;
use zerocopy::{FromBytes, IntoBytes, KnownLayout, U16, U32, U64};
use crate::virtio::OsHal;

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
		if capacity < 100 {
			return Err(FileSystemError::FormatFailed);
		}
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
		println!("[FS] Mounting File System");
		let mut buffer = [0u8; BLOCK_SIZE];

		device
			.read_blocks(SUPERBLOCK_BLOCK, &mut buffer)
			.map_err(|_| FileSystemError::InvalidSuperBlock)?;

		println!("[FS] Read into buffer {:?}", buffer);

		let size = size_of::<DiskSuperBlock>();
		let disk_superblock = DiskSuperBlock::ref_from_bytes(&buffer[..size])
			.map_err(|_| FileSystemError::InvalidSuperBlock)?;

		let superblock = SuperBlock::try_from(*disk_superblock)
			.map_err(|_| FileSystemError::InvalidSuperBlock)?;

		if superblock.magic_number != MAGIC_NUMBER {
			println!("[FS] Superblock magic number match failed");
			return Err(FileSystemError::InvalidSuperBlock);
		}

		// basic sanity checks so that we stay in range
		if superblock.inode_count % (INODES_PER_BLOCK as u64) != 0 {
			return Err(FileSystemError::InvalidSuperBlock);
		}
		if superblock.inode_table_start_block != INODE_TABLE_START_BLOCK {
			return Err(FileSystemError::InvalidSuperBlock);
		}
		if superblock.inode_bitmap_block != INODE_BITMAP_BLOCK
			|| superblock.data_bitmap_block != DATA_BITMAP_BLOCK
		{
			return Err(FileSystemError::InvalidSuperBlock);
		}
		if superblock.data_block_start <= superblock.inode_table_start_block {
			return Err(FileSystemError::InvalidSuperBlock);
		}
		if superblock.data_block_start + superblock.data_block_count > superblock.total_blocks {
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

		if free_inode_index >= self.superblock.inode_count as usize {
			// out of bounds index is allocated .. not gonna work
			return Err(FileSystemError::NoSpace);
		}
		// so the write_blocks of the BlockDevice should be able to overwrite the contents of the
		// block if any exists
		self.device
			.write_blocks(self.superblock.inode_bitmap_block, &bitmap_buffer)
			.map_err(|_| FileSystemError::BlockError)?;

		Ok(free_inode_index as u64)
	}

	/// Allocates a data block following a read-modify-write pattern
	pub fn allocate_data_block(&mut self) -> Result<u64, FileSystemError> {
		// TODO: maybe wrap the u64 here with something similar to FileHandler .. better to have
		let mut bm_buffer = [0u8; BLOCK_SIZE];

		self.device
			.read_blocks(DATA_BITMAP_BLOCK, &mut bm_buffer)
			.map_err(|_| FileSystemError::BlockError)?;

		let mut data_bitmap = Bitmap::new(&mut bm_buffer);

		let free_idx = data_bitmap.find_and_set_first_free().ok_or(FileSystemError::NoSpace)?;

		if free_idx >= self.superblock.data_block_count as usize {
			return Err(FileSystemError::NoSpace);
		}

		self.device
			.write_blocks(DATA_BITMAP_BLOCK, &bm_buffer)
			.map_err(|_| FileSystemError::BlockError)?;

		// find position in the table
		let abs_block = self.superblock.data_block_start + free_idx as u64;

		// zeroing the new block to avoid stale data
		let zero_block = [0u8; BLOCK_SIZE];
		self.device
			.write_blocks(abs_block, &zero_block)
			.map_err(|_| FileSystemError::BlockError)?;

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
			name: [0u8; DIR_NAME_MAX],
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
					println!("[FS] File with same name found");
					return Err(FileSystemError::AlreadyExists); // use FileError::FileExists at
					// call site
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

	fn create_file_in_dir(
		&mut self,
		parent_inode_num: u64,
		filename: &str,
	) -> Result<(u64 /*new inode index*/, u64 /*parent dir data block*/), FileSystemError> {
		if filename.as_bytes().len() > DIR_NAME_MAX || filename.is_empty() {
			return Err(FileSystemError::NameTooLong);
		}

		let parent_dir_inode = self.read_inode(parent_inode_num)?;
		if parent_dir_inode.mode != FileType::Directory {
			// Can't create a file inside a file!
			return Err(FileSystemError::CorruptLayout);
		}

		// get the parent's data block
		// (assumes one block per dir for now)
		let parent_dir_data_block = parent_dir_inode.direct_pointers[0];
		if parent_dir_data_block == 0 {
			// This shouldn't happen for a correctly initialized directory
			return Err(FileSystemError::CorruptLayout);
		}

		let mut parent_dir_block_buf = [0u8; BLOCK_SIZE];
		self.device
			.read_blocks(parent_dir_data_block, &mut parent_dir_block_buf)
			.map_err(|_| FileSystemError::BlockError)?;

		// collision check
		let mut empty_slot_index: Option<usize> = None;
		let entries = DirEntryBlock::new(&parent_dir_block_buf);
		for (i, entry) in entries.enumerate() {
			let is_used = (entry.flags.get() & DIRENT_USED) != 0;
			if is_used {
				let entry_name_len = entry.name_len.get() as usize;
				if &entry.name[..entry_name_len] == filename.as_bytes() {
					println!("[FS] File with same name found");
					return Err(FileSystemError::AlreadyExists);
				}
			} else if empty_slot_index.is_none() {
				empty_slot_index = Some(i);
			}
		}

		let slot_index = empty_slot_index.ok_or(FileSystemError::NoSpace)?;

		let new_inode_index = self.allocate_inode()?;
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
		self.write_inode(new_inode, new_inode_index)?;

		self.write_dirent_into_block(
			&mut parent_dir_block_buf,
			slot_index,
			new_inode_index,
			filename.as_bytes(),
		)?;

		self.device
			.write_blocks(parent_dir_data_block, &parent_dir_block_buf)
			.map_err(|_| FileSystemError::BlockError)?;

		Ok((new_inode_index, parent_dir_data_block))
	}

	fn create_directory_in_root(&mut self, name: &str) -> Result<u64, FileSystemError> {
		if name.as_bytes().len() > DIR_NAME_MAX || name.is_empty() {
			return Err(FileSystemError::NameTooLong);
			// should also include empty one but meh .. fix sometime later
		}

		let root_dir_inode = self.read_inode(ROOT_DIRECTORY_INODE)?;
		if root_dir_inode.mode != FileType::Directory {
			return Err(FileSystemError::CorruptLayout);
		}

		let dir_block_addr = root_dir_inode.direct_pointers[0];
		if dir_block_addr == 0 {
			return Err(FileSystemError::CorruptLayout);
		}

		let mut dir_block_buf = [0u8; BLOCK_SIZE];
		self.device
			.read_blocks(dir_block_addr, &mut dir_block_buf)
			.map_err(|_| FileSystemError::BlockError)?;

		// collision check
		let mut empty_slot_index: Option<usize> = None;
		let entries = DirEntryBlock::new(&dir_block_buf);
		for (i, entry) in entries.enumerate() {
			let is_used = (entry.flags.get() & DIRENT_USED) != 0;
			if is_used {
				let entry_name_len = entry.name_len.get() as usize;
				if &entry.name[..entry_name_len] == name.as_bytes() {
					println!("[FS] Directory with same name found");
					return Err(FileSystemError::AlreadyExists);
				}
			} else if empty_slot_index.is_none() {
				empty_slot_index = Some(i);
			}
		}
		let slot_index = empty_slot_index.ok_or(FileSystemError::NoSpace)?;

		// gotta allocate an inode and data block
		let new_inode_index = self.allocate_inode()?;
		let new_data_block = self.allocate_data_block()?;

		let mut new_inode = Inode {
			mode: FileType::Directory,
			user_id: 0,
			group_id: 0,
			link_count: 2, // Starts with 2 (for . and parent's link to it)
			size_in_bytes: 0, // Dirs don't really use this
			last_access_time: 0,
			last_modification_time: 0,
			creation_time: 0,
			direct_pointers: [0u64; 10],
			indirect_pointer: 0,
		};

		// point new dir's inode to its new data block
		new_inode.direct_pointers[0] = new_data_block;
		self.write_inode(new_inode, new_inode_index)?;

		let mut new_dir_block = [0u8; BLOCK_SIZE];
		// "." points to itself
		self.write_dirent_into_block(&mut new_dir_block, 0, new_inode_index, b".")?;
		// ".." points to its parent (the root)
		self.write_dirent_into_block(&mut new_dir_block, 1, ROOT_DIRECTORY_INODE, b"..")?;

		self.device
			.write_blocks(new_data_block, &new_dir_block)
			.map_err(|_| FileSystemError::BlockError)?;

		self.write_dirent_into_block(&mut dir_block_buf, slot_index, new_inode_index, name.as_bytes())?;

		self.device
			.write_blocks(dir_block_addr, &dir_block_buf)
			.map_err(|_| FileSystemError::BlockError)?;

		println!("[FS] Created directory '{}' with inode #{}", name, new_inode_index);
		Ok(new_inode_index)
	}

	fn find_entry_in_dir(
		&mut self,
		dir_inode: &Inode,
		name: &str,
	) -> Result<u64, FileError> {
		if dir_inode.mode != FileType::Directory {
			return Err(FileError::Corrupt);
		}

		let data_block_addr = dir_inode.direct_pointers[0];
		if data_block_addr == 0 {
			return Err(FileError::FileNotFound);
		}

		let mut dir_block = [0u8; BLOCK_SIZE];
		self.device
			.read_blocks(data_block_addr, &mut dir_block)
			.map_err(|_| FileError::BlockReadError)?;

		let entries = DirEntryBlock::new(&dir_block);
		for e in entries {
			let used = (e.flags.get() & DIRENT_USED) != 0;
			if used {
				let nlen = e.name_len.get() as usize;
				if &e.name[..nlen] == name.as_bytes() {
					return Ok(e.inode.get());
				}
			}
		}
		// we went through the whole block but did not find it
		Err(FileError::FileNotFound)
	}

	fn find_inode_by_path(
		&mut self,
		path: &str,
	) -> Result<u64, FileError> {
		if path == "/" {
			return Ok(ROOT_DIRECTORY_INODE);
		}

		let mut current_inode_num = ROOT_DIRECTORY_INODE;
		let components = path.split('/').filter(|&s| !s.is_empty());

		for comp in components {
			let current_dir_inode = self
				.read_inode(current_inode_num)
				.map_err(|_| FileError::Corrupt)?;

			let next_inode_num = self.find_entry_in_dir(&current_dir_inode, comp)?;
			current_inode_num = next_inode_num;
		}

		Ok(current_inode_num)
	}

	/// Splits a full path into its parent directory and its final component (basename).
	///
	/// Returns `(parent_path, basename)`.
	///
	/// ### Examples
	/// * `/foo/bar.txt` -> `("/foo", "bar.txt")`
	/// * `/foo.txt`     -> `("/", "foo.txt")`
	/// * `foo.txt`      -> `(".", "foo.txt")` (Assumes relative to current dir)
	/// * `/`            -> `("/", "")`
	fn parse_path(path: &str) -> (&str, &str) {
		if path == "/" {
			return ("/", "");
		}

		match path.rsplit_once('/') {
			Some((parent, basename)) => {
				if parent.is_empty() {
					("/", basename)
				} else {
					(parent, basename)
				}
			}
			None => {
				(".", path)
			}
		}
	}

	// free helpers
	fn free_inode(
		&mut self,
		inode_index: u64,
	) -> Result<(), FileSystemError> {
		let mut buffer = [0u8; BLOCK_SIZE];
		self.device
			.read_blocks(INODE_BITMAP_BLOCK, &mut buffer)
			.map_err(|_| FileSystemError::BlockError)?;
		{
			// clear the bit in inode bitmap block using bitmap
			let mut bm = Bitmap::new(&mut buffer);
			bm.clear(inode_index as usize);
		}

		self.device
			.write_blocks(INODE_BITMAP_BLOCK, &mut buffer)
			.map_err(|_| FileSystemError::BlockError)?;

		// now that we have removed the inode bitmap in its table
		// we need to remove the inode now
		// this is the entire block
		let in_block =
			self.superblock.inode_table_start_block + (inode_index / INODES_PER_BLOCK as u64);
		// this is location of the specific inode entry we have to clear
		let offset = (inode_index % INODES_PER_BLOCK as u64) as usize * INODE_SIZE;

		let mut in_buf = [0u8; BLOCK_SIZE];
		self.device
			.read_blocks(in_block, &mut in_buf)
			.map_err(|_| FileSystemError::BlockError)?;
		for b in &mut in_buf[offset..offset + INODE_SIZE] {
			*b = 0; // zero out the bits in this specific inode
		}
		self.device
			.write_blocks(in_block, &in_buf)
			.map_err(|_| FileSystemError::BlockError)?;

		Ok(())
	}

	fn free_data_block(
		&mut self,
		abs_block: u64,
	) -> Result<(), FileSystemError> {
		// abs block is the absolute block index
		if abs_block < self.superblock.data_block_start {
			return Ok(());
		}
		// get the relative data block from the data block start
		let rel = abs_block - self.superblock.data_block_start;
		let mut buf = [0u8; BLOCK_SIZE]; // to read the block into
		self.device
			.read_blocks(DATA_BITMAP_BLOCK, &mut buf)
			.map_err(|_| FileSystemError::BlockError)?;
		{
			let mut bmp = Bitmap::new(&mut buf);
			bmp.clear(rel as usize);
		}
		self.device
			.write_blocks(DATA_BITMAP_BLOCK, &buf)
			.map_err(|_| FileSystemError::BlockError)?;

		Ok(())
	}

	fn find_root_entry(
		&mut self,
		name: &str,
	) -> Result<(usize, u64 /*inode*/, [u8; BLOCK_SIZE], u64 /*dir block*/), FileSystemError> {
		let root_inode = self.read_inode(0)?;
		let blk = root_inode.direct_pointers[0];
		if blk == 0 {
			return Err(FileSystemError::CorruptLayout);
		}

		let mut dir_block = [0u8; BLOCK_SIZE];
		self.device
			.read_blocks(blk, &mut dir_block)
			.map_err(|_| FileSystemError::BlockError)?;

		let entries = DirEntryBlock::new(&dir_block);
		for (i, e) in entries.enumerate() {
			let used = (e.flags.get() & DIRENT_USED) != 0;
			if used {
				let nlen = e.name_len.get() as usize;
				if &e.name[..nlen] == name.as_bytes() {
					return Ok((i, e.inode.get(), dir_block, blk));
				}
			}
		}

		Err(FileSystemError::CorruptLayout)
	}

	fn find_entry_detailed(
		&mut self,
		parent_inode_num: u64,
		filename: &str,
	) -> Result<(
		usize, /* slot index */
		u64,   /* file inode num */
		[u8; BLOCK_SIZE], /* parent dir block buffer */
		u64    /* parent dir block address */
	), FileError> {
		let parent_inode = self.read_inode(parent_inode_num).map_err(|_| FileError::Corrupt)?;
		if parent_inode.mode != FileType::Directory {
			return Err(FileError::Corrupt);
		}

		let dir_block_addr = parent_inode.direct_pointers[0];
		if dir_block_addr == 0 {
			return Err(FileError::FileNotFound);
		}

		let mut dir_block_buf = [0u8; BLOCK_SIZE];
		self.device
			.read_blocks(dir_block_addr, &mut dir_block_buf)
			.map_err(|_| FileError::BlockReadError)?;

		let entries = DirEntryBlock::new(&dir_block_buf);
		for (i, e) in entries.enumerate() {
			let used = (e.flags.get() & DIRENT_USED) != 0;
			if used {
				let nlen = e.name_len.get() as usize;
				if &e.name[..nlen] == filename.as_bytes() {
					// Found it!
					return Ok((i, e.inode.get(), dir_block_buf, dir_block_addr));
				}
			}
		}

		Err(FileError::FileNotFound)
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
	NameTooLong,
}

pub trait FileSystem {
	/// create a file in the File System
	fn create_file(
		&mut self,
		name: &str,
	) -> Result<FileHandler, FileError>;
	/// Delete a file from the File System
	fn delete_file(
		&mut self,
		path: &str,
	) -> Result<(), FileError>;
	/// Open a file from the file system for reading or writing
	fn open_file(
		&mut self,
		name: &str,
	) -> Result<FileHandler, FileError>;
	/// List the available files
	fn list_file(&mut self, path: &str) -> Result<Vec<String>, FileError>;
}

#[derive(Debug)]
pub enum FileSystemError {
	AlreadyExists,
	BlockError,
	CorruptLayout,
	FormatFailed,
	InvalidSuperBlock,
	MountFailed,
	NameTooLong,
	NoSpace,
}

impl<D: BlockDevice> FileSystem for SFS<D> {
	fn create_file(
		&mut self,
		path: &str,
	) -> Result<FileHandler, FileError> {
		// "/docs/new.txt" -> ("/docs", "new.txt")
		let (parent_path, basename) = Self::parse_path(path);
		if basename.is_empty() {
			return Err(FileError::InvalidName); // Can't create a file with no name
		}

		let parent_inode_num = self.find_inode_by_path(parent_path)?;

		let (new_inode_index, _dir_block) = self
			.create_file_in_dir(parent_inode_num, basename)
			.map_err(|e| match e {
				FileSystemError::NameTooLong => FileError::InvalidName,
				FileSystemError::NoSpace => FileError::NoSpace,
				FileSystemError::CorruptLayout => FileError::Corrupt,
				FileSystemError::AlreadyExists => FileError::FileExists,
				_ => FileError::CreationFailed,
			})?;

		println!("[FS] Created file '{}' with inode #{}", path, new_inode_index);
		Ok(FileHandler(new_inode_index as usize))
	}

	fn delete_file(
		&mut self,
		path: &str,
	) -> Result<(), FileError> {
		let (parent_path, basename) = Self::parse_path(path);
		if basename.is_empty() || basename == "." || basename == ".." {
			return Err(FileError::InvalidName);
		}

		let parent_inode_num = self.find_inode_by_path(parent_path)?;

		let (slot, inode_idx, mut dir_block, dir_blk_addr) =
			self.find_entry_detailed(parent_inode_num, basename)?;

		if inode_idx == 0 { // should not happen
			return Err(FileError::FileNotFound);
		}

		let inode = self.read_inode(inode_idx).map_err(|_| FileError::Corrupt)?;
		// TODO: add check here so that we don't delete a non-empty directory - not necessary now!
		if inode.mode == FileType::Directory {
			return Err(FileError::InvalidName); // is a directory
		}

		for &dp in &inode.direct_pointers {
			if dp != 0 {
				self.free_data_block(dp).map_err(|_| FileError::Corrupt)?;
			}
		}

		// free the inode
		self.free_inode(inode_idx).map_err(|_| FileError::Corrupt)?;

		// clearing the directory entries here
		let start = slot * DIR_ENTRY_SIZE;
		for b in &mut dir_block[start..start + DIR_ENTRY_SIZE] {
			*b = 0;
		}

		self.device
			.write_blocks(dir_blk_addr, &dir_block)
			.map_err(|_| FileError::BlockWriteError)?;

		println!("[FS] Deleted file '{}' (inode #{})", path, inode_idx);
		Ok(())
	}

	fn open_file(
		&mut self,
		path: &str,
	) -> Result<FileHandler, FileError> {
		let inode_idx = self.find_inode_by_path(path)?;
		Ok(FileHandler(inode_idx as usize))
	}

	/// since we currently follow a single directory structure, this just returns the names of
	/// files in that directory
	fn list_file(
		&mut self,
		path: &str, // The trait now passes us a path
	) -> Result<Vec<String>, FileError> {
		// 1. Find the inode for the directory they want to list
		let dir_inode_num = self.find_inode_by_path(path)?;
		let dir_inode = self.read_inode(dir_inode_num).map_err(|_| FileError::Corrupt)?;

		// 2. Check if it's actually a directory
		if dir_inode.mode != FileType::Directory {
			// You can't 'ls' a file!
			return Err(FileError::InvalidName);
		}

		// 3. Get its data block
		let block = dir_inode.direct_pointers[0];
		if block == 0 {
			return Ok(Vec::new()); // Empty directory
		}

		// 4. Read the block and list its entries (same logic as before)
		let mut buf = [0u8; BLOCK_SIZE];
		self.device
			.read_blocks(block, &mut buf)
			.map_err(|_| FileError::BlockReadError)?;

		let mut res = Vec::new();
		let entries = DirEntryBlock::new(&buf);
		for e in entries {
			let used = (e.flags.get() & DIRENT_USED) != 0;
			if used && e.inode.get() != 0 {
				let nlen = e.name_len.get() as usize;
				if nlen >= DIR_NAME_MAX {
					return Err(FileError::NameTooLong);
				}
				let name_slice = &e.name[..nlen];
				if let Ok(s) = core::str::from_utf8(name_slice) {
					res.push(s.to_string());
				}
			}
		}

		Ok(res)
	}
}

impl<D: BlockDevice> SFS<D> {
	pub fn write_file_lines(
		&mut self,
		handle: FileHandler,
		lines: &[&str], // reference to a string slice? -- string slice is reference right?
	) -> Result<(), FileError> {
		let inode_idx = handle.0 as u64;
		let mut inode = self.read_inode(inode_idx).map_err(|_| FileError::Corrupt)?;
		if inode.mode != FileType::File {
			return Err(FileError::Corrupt);
		}

		// append a new line to whatever the user just added
		let joined = lines.join("\n");
		let bytes = joined.as_bytes();

		if bytes.len() > BLOCK_SIZE {
			return Err(FileError::NoSpace);
			// simple check -- maybe add some proper warning here
			// to the user when he tries to save
		}

		// check for data block allocation
		if inode.direct_pointers[0] == 0 {
			let b = self.allocate_data_block().map_err(|_| FileError::NoSpace)?;
			inode.direct_pointers[0] = b;
		}

		let data_block = inode.direct_pointers[0];
		
		let mut buf = [0u8; BLOCK_SIZE];
		buf[.. bytes.len()].copy_from_slice(bytes);
		
		// wrote into the buffer
		// now have to write this into disk
		self.device
			.write_blocks(data_block, &buf)
			.map_err(|_| FileError::BlockWriteError)?;

		// we also need to update inode metadata
		inode.size_in_bytes = bytes.len() as u64;
		// timestamps for inode are missing
		// not a priority but nice to have
		self.write_inode(inode, inode_idx).map_err(|_| FileError::BlockWriteError)?;
		Ok(())
	}
	
	pub fn read_file(&mut self, handle: FileHandler) -> Result<String, FileError> {
		let inode_idx = handle.0 as u64;
		let inode = self.read_inode(inode_idx).map_err(|_| FileError::Corrupt)?;
		if inode.mode != FileType::File {
			return Err(FileError::Corrupt);
		}
		
		let sz = inode.size_in_bytes as usize;
		if sz == 0 {
			return Ok(String::new());
		}
		
		let data_block = inode.direct_pointers[0];
		if data_block == 0 {
			return Err(FileError::Corrupt);
		}
		
		let mut buf = [0u8; BLOCK_SIZE];
		self.device
			.read_blocks(data_block, &mut buf)
			.map_err(|_| FileError::BlockReadError)?;
		
		let slice = &buf[..sz];
		let s = core::str::from_utf8(slice).map_err(|_| FileError::Corrupt)?;
		// should I make different types of corrupt? 
		// maybe some good verbose output for user would be better -- look for crates for this!
		Ok(s.to_string())
	}
}

// Exact type of the file system that we're going to use
// the memory mapped regions of the VirtIOBlk device live forever?
pub type GLOBALFSType = SFS<VirtIOBlk<OsHal, PciTransport>>; // this does not expect a lifetime