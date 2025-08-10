use crate::fs::simple_fs::FileSystemError;
use alloc::vec::Vec;
use sa::const_assert;
use zerocopy::{
	FromBytes, Immutable, IntoBytes, KnownLayout,
	byteorder::{LE, U16, U32, U64},
};

pub const BLOCK_SIZE: usize = 512;
pub const INODE_SIZE: usize = 128;
pub const INODES_PER_BLOCK: usize = BLOCK_SIZE / INODE_SIZE; // --- 4

// BLOCK ADDRESSES for different sections of the file system
pub const SUPERBLOCK_BLOCK: u64 = 0;
pub const INODE_BITMAP_BLOCK: u64 = 1;
pub const DATA_BITMAP_BLOCK: u64 = 2;
pub const INODE_TABLE_START_BLOCK: u64 = 3;

type U32Le = U32<LE>;

#[derive(Debug, Copy, Clone, IntoBytes, FromBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct DiskSuperBlock {
	pub total_blocks: U64<LE>,
	pub inode_bitmap_block: U64<LE>,
	pub data_bitmap_block: U64<LE>,
	pub inode_table_start_block: U64<LE>,
	pub inode_count: U64<LE>,
	pub data_block_start: U64<LE>,
	pub data_block_count: U64<LE>,
	pub magic_number: U32Le,
	pub _pad0: U32Le, // explicit padding to avoid implicit tail padding so total is 64 bytes
}

#[derive(Debug, Copy, Clone)]
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
}

const_assert!(core::mem::size_of::<DiskSuperBlock>() == 64);
// A single SuperBlock struct fits within a disk
const_assert!(core::mem::size_of::<DiskSuperBlock>() <= BLOCK_SIZE);

impl From<SuperBlock> for DiskSuperBlock {
	fn from(sb: SuperBlock) -> Self {
		DiskSuperBlock {
			total_blocks: U64::new(sb.total_blocks),
			inode_bitmap_block: U64::new(sb.inode_bitmap_block),
			data_bitmap_block: U64::new(sb.data_bitmap_block),
			inode_table_start_block: U64::new(sb.inode_table_start_block),
			inode_count: U64::new(sb.inode_count),
			data_block_start: U64::new(sb.data_block_start),
			data_block_count: U64::new(sb.data_block_count),
			magic_number: U32Le::new(sb.magic_number),
			_pad0: U32Le::new(0),
		}
	}
}

impl core::convert::TryFrom<DiskSuperBlock> for SuperBlock {
	type Error = ();

	fn try_from(value: DiskSuperBlock) -> Result<Self, Self::Error> {
		Ok(SuperBlock {
			total_blocks: value.total_blocks.get(),
			inode_bitmap_block: value.inode_bitmap_block.get(),
			data_bitmap_block: value.data_bitmap_block.get(),
			inode_table_start_block: value.inode_table_start_block.get(),
			inode_count: value.inode_count.get(),
			data_block_start: value.data_block_start.get(),
			data_block_count: value.data_block_count.get(),
			magic_number: value.magic_number.get(),
		})
	}
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct Inode {
	pub mode: FileType,
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

#[derive(Debug, Copy, Clone, IntoBytes, FromBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct DiskInode {
	// 64-bit fields first for natural padding into 128 bytes total
	pub size_in_bytes: U64<LE>,          // 8   | 8
	pub last_access_time: U64<LE>,       // 8   | 16
	pub last_modification_time: U64<LE>, // 8   | 24
	pub creation_time: U64<LE>,          // 8   | 32
	pub direct_pointers: [U64<LE>; 10],  // 80  | 112
	pub indirect_pointer: U64<LE>,       // 8   | 120
	// small fields at the end, no padding if they sum upto 128
	pub mode: U16<LE>,       // 2   | 122
	pub user_id: U16<LE>,    // 2   | 124
	pub group_id: U16<LE>,   // 2   | 126
	pub link_count: U16<LE>, // 2   | 128
}

const_assert!(size_of::<DiskInode>() == 128);

impl From<Inode> for DiskInode {
	fn from(i: Inode) -> Self {
		DiskInode {
			size_in_bytes: U64::new(i.size_in_bytes),
			last_access_time: U64::new(i.last_access_time),
			last_modification_time: U64::new(i.last_modification_time),
			creation_time: U64::new(i.creation_time),
			direct_pointers: i.direct_pointers.map(U64::new),
			indirect_pointer: U64::new(i.indirect_pointer),
			mode: U16::new(u16::from(i.mode)),
			user_id: U16::new(i.user_id),
			group_id: U16::new(i.group_id),
			link_count: U16::new(i.link_count),
		}
	}
}

impl core::convert::TryFrom<DiskInode> for Inode {
	type Error = ();
	fn try_from(di: DiskInode) -> Result<Self, ()> {
		Ok(Inode {
			mode: FileType::try_from(di.mode.get())?,
			user_id: di.user_id.get(),
			group_id: di.group_id.get(),
			link_count: di.link_count.get(),
			size_in_bytes: di.size_in_bytes.get(),
			last_access_time: di.last_access_time.get(),
			last_modification_time: di.last_modification_time.get(),
			creation_time: di.creation_time.get(),
			direct_pointers: di.direct_pointers.map(|v| v.get()),
			indirect_pointer: di.indirect_pointer.get(),
		})
	}
}

const_assert!(core::mem::size_of::<DiskInode>() == INODE_SIZE);

// Shouldn't the bitmap bits also hold which resource block they are pointing to?
// Nope the position of the bitmap is the pointer .. that's the whole point of it!
// A Bitmap is a view over raw bytes
#[derive(Debug)]
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
		// Faster scan: skip fully-allocated bytes (0xFF) first
		for (byte_idx, &byte) in self.map.iter().enumerate() {
			if byte != 0xFF {
				let base = byte_idx * 8;
				for bit in 0..8 {
					let idx = base + bit;
					if (byte & (1 << bit)) == 0 {
						// Bounds check: idx may exceed logical size if map length is not exact
						// Caller should ensure bitmap length maps exactly to resource count
						let _ = self.set(idx).ok()?;
						return Some(idx);
					}
				}
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u16)]
pub enum FileType {
	Unknown = 0,
	File = 0x1,
	Directory = 0x2,
}

impl core::convert::TryFrom<u16> for FileType {
	type Error = ();
	fn try_from(value: u16) -> Result<Self, Self::Error> {
		match value {
			0 => Ok(FileType::Unknown),
			0x1 => Ok(FileType::File),
			0x2 => Ok(FileType::Directory),
			_ => Err(()),
		}
	}
}

impl From<FileType> for u16 {
	fn from(value: FileType) -> Self {
		value as u16
	}
}