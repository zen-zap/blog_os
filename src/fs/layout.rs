use alloc::vec::Vec;
use sa::const_assert;
use zerocopy::{FromBytes, Immutable, IntoBytes};

pub const BLOCK_SIZE: usize = 512;
pub const INODE_SIZE: usize = 128;
pub const INODES_PER_BLOCK: usize = BLOCK_SIZE / INODE_SIZE; // --- 4

// BLOCK ADDRESSES for different sections of the file system
pub const SUPERBLOCK_BLOCK: u64 = 0;
pub const INODE_BITMAP_BLOCK: u64 = 1;
pub const DATA_BITMAP_BLOCK: u64 = 2;
pub const INODE_TABLE_START_BLOCK: u64 = 3;
//pub const INODE_TABLE_BLOCKS: usize = 5;

#[derive(Debug, Copy, Clone, IntoBytes, FromBytes, Immutable)]
#[repr(C)]
pub struct SuperBlock {
	pub total_blocks: u64,
	pub inode_bitmap_block: u64,
	pub data_bitmap_block: u64,
	pub inode_table_start_block: u64,
	pub inode_count: u64,
	pub data_block_start: u64,
	pub data_block_count: u64,
	pub magic_number: u32, // kept at the end .. so there is no alignment padding
	pub _pad0: u32,        // explicit padding
}

const_assert!(core::mem::size_of::<SuperBlock>() == 64);
// A single SuperBlock struct fits within a disk
const_assert!(core::mem::size_of::<SuperBlock>() <= BLOCK_SIZE);

#[derive(Debug, Copy, Clone, IntoBytes, FromBytes, Immutable)]
#[repr(C)]
pub struct Inode {
	pub mode: u16, // File type and permissions
	pub user_id: u16,
	pub group_id: u16,
	pub link_count: u16,
	pub size_in_bytes: u64,
	pub last_access_time: u64,
	pub last_modification_time: u64,
	pub creation_time: u64,
	pub direct_pointers: [u64; 10], // direct pointers for simplicity
	pub indirect_pointer: u64,
}

// Shouldn't the bitmap bits also hold which resource block they are pointing to?
// Nope the position of the bitmap is the pointer .. that's the whole point of it!

#[derive(Debug, IntoBytes, FromBytes, Immutable)]
#[repr(C)]
pub struct Bitmap<'a> {
	pub map: &'a mut [u8],
}
// used the ceiling function to calculate the minimum number of bytes required to store this

const_assert!(core::mem::size_of::<Bitmap>() <= BLOCK_SIZE);

impl<'a> Bitmap<'a> {
	/// creates a new bitmap overlaying a mutable byte slice
	pub fn new(map: &'a mut [u8]) -> Self {
		Self { map }
	}

	/// Takes the bit index not the byte index
	/// Checks if the bit at a given index is set to 1
	pub fn is_set(
		&self,
		idx: usize,
	) -> bool {
		let byte_index = idx / 8;
		let bit_index = idx % 8;

		(self.map[byte_index] & (1 << bit_index)) != 0
		// this is bit-masking
		// standard way to access individual bits in an array
	}

	/// Sets the bit at a given value of 1
	/// Returns an error if already set
	pub fn set(
		&mut self,
		idx: usize,
	) -> Result<(), BitmapError> {
		if self.is_set(idx) {
			return Err(BitmapError::AlreadyAllocated);
		}
		let byte_index = idx / 8;
		let bit_index = idx % 8;
		self.map[byte_index] |= 1 << bit_index;
		Ok(())
	}

	/// Clears the bit at a given index to 0.
	/// Returns an error if it was already clear.
	pub fn clear(
		&mut self,
		idx: usize,
	) -> Result<(), BitmapError> {
		if !self.is_set(idx) {
			return Err(BitmapError::AlreadyCleared);
		}

		let byte_index = idx / 8;
		let bit_index = idx % 8;
		self.map[byte_index] &= !(1 << bit_index);
		Ok(())
	}

	pub fn find_and_set_first_free(&mut self) -> Option<usize> {
		for i in 0..(self.map.len() * 8) {
			if !self.is_set(i) {
				self.set(i).unwrap();
				return Some(i);
			}
		}

		None
	}
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(C)]
pub enum BitmapError {
	AlreadyAllocated,
	AlreadyCleared,
}