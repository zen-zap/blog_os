//! in src/fs/block_handler.rs

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

// the start points in KB of each block
const SUPERBLOCK_BLOCK: u64 = 0;
const INODE_BITMAP_BLOCK: u64 = 1;
const DATA_BITMAP_BLOCK: u64 = 2;
const INODE_START_BLOCK: u64 = 3;
const INODE_BLOCK_COUNT: u64 = 5;
const DATA_START_BLOCK: u64 = INODE_START_BLOCK + INODE_BLOCK_COUNT; // = 8

/// Contains File System information
pub struct SuperBlock {
	magic: u32,
	version: u32,
	total_blocks: u64,
	inode_bitmap_block: u64,
	data_bitmap_block: u64,
	inode_table_start: u64,
	inode_table_blocks: u64,
	data_start: u64,
}

/// Contains information about availability of Inodes
pub struct InodeBitmap {
	pub map: Vec<u8>,
}

/// Contains information about availability of Data Blocks
pub struct DataBitmap {
	pub map: Vec<u8>,
}

/// Represents the Inode table
pub struct InodeTable {
	pub num_blocks: usize,
	pub start_addr: usize,
	pub inode_blocks: Vec<InodeBlock>,
}

pub struct InodeBlock {
	pub inodes: Vec<Inode>,
}

// TODO: Add support for larger files later on with multi-level indexing

/// Represents a single Inode
///
/// Holds the metadata for a file stored in the Data Region
pub struct Inode {
	pub i_num: usize,
	pub size: usize, // default to 256 bytes
	pub file_type: SimFile,
	// TODO: Add other parameters like time, owners, permission, version, etc. later on
}

/// Represents whether current file is a normal file, directory, etc.
pub enum SimFile {
	File,
	//Directory, // TODO: this would need to hold a file tree so implement this later
}

pub struct DataRegion {
	num_blocks: usize,
	block_size: usize,
	blocks: Vec<DataBlocks>,
}

pub struct DataBlocks {
	data: String,
}