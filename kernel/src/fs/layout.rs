//! Filesystem on-disk data structures and layout definitions.
//!
//! This module defines all the data structures that are stored on disk in the SFS filesystem,
//! as well as constants that govern the filesystem layout. It uses the `zerocopy` crate to
//! ensure safe serialization between in-memory and on-disk representations.
//!
//! # Disk Layout Overview
//!
//! ```text
//! Block 0: Superblock (metadata about the filesystem)
//! Block 1: Inode Bitmap (tracks allocated inodes)
//! Block 2: Data Bitmap (tracks allocated data blocks)
//! Block 3+: Inode Table (10% of disk, 4 inodes per block)
//! Remaining: Data Blocks (90% of disk, file/directory contents)
//! ```
//!
//! # Key Concepts
//!
//! - **Disk vs. In-Memory Structures**: Each structure has two versions:
//!   - `Disk*` types use little-endian byte order (zerocopy types like `U64<LE>`)
//!   - In-memory types use native byte order (standard Rust types like `u64`)
//! - **Bitmaps**: Track resource allocation using bit arrays
//! - **Inodes**: Store file/directory metadata and block pointers
//! - **Directory Entries**: Map filenames to inode numbers

// TODO: Documentation added by copilot -- verify them!
use crate::fs::simple_fs::FileSystemError;
use alloc::vec::Vec;
use sa::const_assert;
use zerocopy::{
	FromBytes, Immutable, IntoBytes, KnownLayout,
	byteorder::{LE, U16, U32, U64},
};

/// Size of a single disk block in bytes.
///
/// This matches the standard sector size for most block devices and ensures
/// efficient I/O operations. All filesystem structures are designed to align
/// with this block size.
pub const BLOCK_SIZE: usize = 512;

/// Size of a single inode structure in bytes.
///
/// Each inode is 128 bytes, allowing exactly 4 inodes to fit in one 512-byte block.
/// This size accommodates all inode metadata including 10 direct pointers and
/// 1 indirect pointer.
pub const INODE_SIZE: usize = 128;

/// Number of inodes that fit in a single block.
///
/// Calculated as BLOCK_SIZE / INODE_SIZE = 512 / 128 = 4.
/// This determines how inode indices map to block numbers in the inode table.
pub const INODES_PER_BLOCK: usize = BLOCK_SIZE / INODE_SIZE; // --- 4

// BLOCK ADDRESSES for different sections of the file system

/// Block address where the superblock is stored.
///
/// The superblock is always at block 0 and contains critical filesystem metadata
/// like total size, inode count, and layout information.
pub const SUPERBLOCK_BLOCK: u64 = 0;

/// Block address where the inode allocation bitmap is stored.
///
/// This bitmap tracks which inodes are allocated (1) or free (0).
/// Each bit corresponds to one inode number.
pub const INODE_BITMAP_BLOCK: u64 = 1;

/// Block address where the data block allocation bitmap is stored.
///
/// This bitmap tracks which data blocks are allocated (1) or free (0).
/// Each bit corresponds to one data block in the data region.
pub const DATA_BITMAP_BLOCK: u64 = 2;

/// Starting block address for the inode table.
///
/// The inode table begins at block 3 and extends for 10% of the disk capacity.
/// Each block contains 4 inodes (INODES_PER_BLOCK).
pub const INODE_TABLE_START_BLOCK: u64 = 3;

// Directory Entry Layout: 64 bytes per entry -> 8 entries per 512 block

/// Size of a single directory entry in bytes.
///
/// Each entry is 64 bytes, allowing 8 entries per 512-byte block.
/// This size accommodates an inode number, name length, flags, and a 52-byte filename.
pub const DIR_ENTRY_SIZE: usize = 64;

/// Maximum length of a filename in bytes.
///
/// Filenames can be up to 52 bytes (52 characters if ASCII).
/// This limit ensures directory entries fit within DIR_ENTRY_SIZE.
pub const DIR_NAME_MAX: usize = 52;

/// Number of directory entries that fit in a single block.
///
/// Calculated as BLOCK_SIZE / DIR_ENTRY_SIZE = 512 / 64 = 8.
/// This determines how many files/subdirectories a single directory block can hold.
pub const DIR_ENTRIES_PER_BLOCK: usize = BLOCK_SIZE / DIR_ENTRY_SIZE;

/// Type alias for little-endian 32-bit unsigned integer.
///
/// Used in disk structures to ensure consistent byte ordering across different
/// architectures. All disk structures use little-endian format.
type U32Le = U32<LE>;

/// On-disk representation of the filesystem superblock with little-endian byte order.
///
/// This structure is stored at block 0 and contains metadata about the entire filesystem.
/// Uses zerocopy types (U64<LE>, U32<LE>) for safe serialization to/from disk.
/// Size is exactly 64 bytes to fit cleanly within a disk block.
#[derive(Debug, Copy, Clone, IntoBytes, FromBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct DiskSuperBlock {
	/// Total number of blocks on the device (disk capacity).
	pub total_blocks: U64<LE>,
	/// Block number where the inode bitmap is stored (always 1).
	pub inode_bitmap_block: U64<LE>,
	/// Block number where the data bitmap is stored (always 2).
	pub data_bitmap_block: U64<LE>,
	/// Block number where the inode table begins (always 3).
	pub inode_table_start_block: U64<LE>,
	/// Total number of inodes available in the filesystem.
	pub inode_count: U64<LE>,
	/// Block number where the data region begins (after inode table).
	pub data_block_start: U64<LE>,
	/// Number of blocks in the data region.
	pub data_block_count: U64<LE>,
	/// Magic number for filesystem identification (0xDEADBEEF).
	pub magic_number: U32Le,
	/// Explicit padding to reach exactly 64 bytes, avoiding implicit tail padding.
	pub _pad0: U32Le, // explicit padding to avoid implicit tail padding so total is 64 bytes
}

/// In-memory representation of the filesystem superblock with native byte order.
///
/// This is the working copy used by the filesystem implementation.
/// Can be converted to/from DiskSuperBlock for persistence.
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct SuperBlock {
	/// Total number of blocks on the device (disk capacity).
	pub total_blocks: u64,
	/// Block number where the inode bitmap is stored (always 1).
	pub inode_bitmap_block: u64,
	/// Block number where the data bitmap is stored (always 2).
	pub data_bitmap_block: u64,
	/// Block number where the inode table begins (always 3).
	pub inode_table_start_block: u64,
	/// Total number of inodes available in the filesystem.
	pub inode_count: u64,
	/// Block number where the data region begins (after inode table).
	pub data_block_start: u64,
	/// Number of blocks in the data region.
	pub data_block_count: u64,
	/// Magic number for filesystem identification (0xDEADBEEF).
	/// Kept at the end to avoid alignment padding.
	pub magic_number: u32, // kept at the end .. so there is no alignment padding
}

const_assert!(core::mem::size_of::<DiskSuperBlock>() == 64);
// A single SuperBlock struct fits within a disk
const_assert!(core::mem::size_of::<DiskSuperBlock>() <= BLOCK_SIZE);

/// Converts an in-memory SuperBlock to its on-disk representation.
///
/// This conversion handles the transformation from native byte order to little-endian
/// format for disk storage. All numeric fields are wrapped in zerocopy types.
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

/// Converts an on-disk DiskSuperBlock to its in-memory representation.
///
/// This conversion handles the transformation from little-endian byte order to native
/// format for in-memory use. All zerocopy types are unwrapped to standard Rust types.
///
/// # Returns
/// * `Ok(SuperBlock)` - Successfully converted superblock
/// * `Err(())` - Conversion failed (currently this never fails, but reserved for future validation)
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

/// In-memory representation of a filesystem inode (index node).
///
/// An inode contains metadata about a file or directory, including permissions,
/// timestamps, size, and pointers to the actual data blocks on disk.
/// Supports up to 10 direct blocks plus single indirect addressing.
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct Inode {
	/// Type of filesystem entry (File, Directory, Unknown).
	pub mode: FileType,
	/// User ID of the file owner (currently unused, set to 0).
	pub user_id: u16,
	/// Group ID of the file owner (currently unused, set to 0).
	pub group_id: u16,
	/// Number of hard links to this inode (1 for files, 2+ for directories).
	pub link_count: u16,
	/// Size of the file content in bytes (not used for directories).
	pub size_in_bytes: u64,
	/// Timestamp of last access (currently unused, set to 0).
	pub last_access_time: u64,
	/// Timestamp of last modification (currently unused, set to 0).
	pub last_modification_time: u64,
	/// Timestamp of inode creation (currently unused, set to 0).
	pub creation_time: u64,
	/// each entry is the block number of a data block that contains file contents
	///
	/// Array of 10 direct block pointers.
	///
	/// Each entry contains the absolute block number of a data block that holds
	/// file or directory contents. With BLOCK_SIZE=512, this supports files up to
	/// 5120 bytes (10 * 512) without needing indirect pointers.
	///
	/// A value of 0 indicates an unused pointer (sentinel value).
	pub direct_pointers: [u64; 10],
	/// single block number that points to an indirect block.
	/// The indirect block contains an array of block numbers pointing to additional data blocks.
	/// That extends the maximum file size without enlarging the inode.
	///
	/// pointer value = 0 = sentinel (unused)
	///
	/// Single indirect block pointer for extending file size.
	///
	/// Points to a data block that contains an array of block numbers (up to 64 entries
	/// since each u64 is 8 bytes and BLOCK_SIZE=512). Each of these block numbers points
	/// to an actual data block containing file contents.
	///
	/// This extends maximum file size from 5KB (direct only) to ~37KB (direct + indirect).
	///
	/// # Calculation
	/// - Direct blocks: 10 * 512 = 5,120 bytes
	/// - Indirect blocks: (512 / 8) * 512 = 64 * 512 = 32,768 bytes
	/// - Total: 37,888 bytes maximum file size
	///
	/// A value of 0 indicates the indirect block is not allocated (sentinel value).
	pub indirect_pointer: u64,
}

/// On-disk representation of an inode with little-endian byte order.
///
/// This structure is exactly 128 bytes and uses zerocopy types for safe serialization.
/// Fields are ordered carefully to avoid padding: 64-bit fields first, then smaller fields.
/// Multiple inodes are packed into a single block (4 inodes per 512-byte block).
#[derive(Debug, Copy, Clone, IntoBytes, FromBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct DiskInode {
	// 64-bit fields first for natural padding into 128 bytes total
	/// Size of the file content in bytes.
	pub size_in_bytes: U64<LE>, // 8   | 8
	/// Timestamp of last access (Unix timestamp).
	pub last_access_time: U64<LE>, // 8   | 16
	/// Timestamp of last modification (Unix timestamp).
	pub last_modification_time: U64<LE>, // 8   | 24
	/// Timestamp of inode creation (Unix timestamp).
	pub creation_time: U64<LE>, // 8   | 32
	/// Array of 10 direct block pointers (absolute block numbers).
	pub direct_pointers: [U64<LE>; 10], // 80  | 112
	/// Single indirect block pointer (absolute block number).
	pub indirect_pointer: U64<LE>, // 8   | 120
	// small fields at the end, no padding if they sum upto 128
	/// File type (File=1, Directory=2, Unknown=0).
	pub mode: U16<LE>, // 2   | 122
	/// User ID of the owner.
	pub user_id: U16<LE>, // 2   | 124
	/// Group ID of the owner.
	pub group_id: U16<LE>, // 2   | 126
	/// Number of hard links to this inode.
	pub link_count: U16<LE>, // 2   | 128
}

const_assert!(size_of::<DiskInode>() == 128);

/// Converts an in-memory Inode to its on-disk representation.
///
/// This conversion wraps all numeric fields in little-endian zerocopy types
/// and converts the FileType enum to its u16 representation.
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

/// Converts an on-disk DiskInode to its in-memory representation.
///
/// This conversion unwraps all zerocopy types to native byte order and
/// validates the FileType enum value.
///
/// # Returns
/// * `Ok(Inode)` - Successfully converted inode
/// * `Err(())` - Conversion failed (invalid FileType value)
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

/// On-disk representation of a directory entry with little-endian byte order.
///
/// Each directory entry is exactly 64 bytes, allowing 8 entries per 512-byte block.
/// The entry maps a filename to an inode number and includes flags to indicate validity.
#[derive(Debug, Copy, Clone, IntoBytes, FromBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct DiskDirEntry {
	/// target inode number
	///
	/// The inode number this directory entry points to. A value of 0 indicates
	/// an unused or deleted entry.
	pub inode: U64<LE>,
	/// length of name in bytes
	///
	/// The actual length of the filename stored in the `name` field.
	/// Must be <= DIR_NAME_MAX (52 bytes).
	pub name_len: U16<LE>,
	/// bit 0 = USED
	///
	/// Flags indicating the entry's state. Bit 0 (DIRENT_USED) indicates whether
	/// this entry is currently in use. Other bits reserved for future use.
	pub flags: U16<LE>,
	/// Fixed-size buffer for the filename (52 bytes).
	///
	/// Only the first `name_len` bytes contain valid filename data.
	/// Unused bytes should be zeroed but are not guaranteed to be.
	pub name: [u8; DIR_NAME_MAX],
}

/// Helper struct to iterate over different DiskDirEntries in a buffer.
///
/// Provides an iterator interface over the directory entries packed into a single block.
/// Each call to `next()` returns the next entry until all entries in the block are exhausted.
pub struct DirEntryBlock<'a> {
	/// Reference to the block buffer containing packed directory entries.
	block: &'a [u8; BLOCK_SIZE],
	/// Current iteration index (0-7 for an 8-entry block).
	idx: usize,
}

impl<'a> DirEntryBlock<'a> {
	/// Creates a new directory entry iterator from a block buffer.
	///
	/// # Arguments
	/// * `block` - A 512-byte block buffer containing packed directory entries
	///
	/// # Returns
	/// A new `DirEntryBlock` iterator starting at index 0
	pub fn new(block: &'a [u8; BLOCK_SIZE]) -> Self {
		Self { block, idx: 0 }
	}
}

impl<'a> Iterator for DirEntryBlock<'a> {
	type Item = DiskDirEntry;

	/// Returns the next directory entry in the block.
	///
	/// # Returns
	/// * `Some(DiskDirEntry)` - The next entry if within bounds and parseable
	/// * `None` - If all entries have been consumed or parsing fails
	///
	/// # Implementation
	/// Extracts the entry at the current index by:
	/// 1. Calculating byte offset: `idx * DIR_ENTRY_SIZE`
	/// 2. Deserializing the 64-byte slice to `DiskDirEntry`
	/// 3. Incrementing the index for next call
	fn next(&mut self) -> Option<Self::Item> {
		if self.idx >= DIR_ENTRIES_PER_BLOCK {
			return None;
		}

		let start = self.idx * DIR_ENTRY_SIZE;
		let end = start + DIR_ENTRY_SIZE;
		let entry = DiskDirEntry::ref_from_bytes(&self.block[start..end]).ok()?;

		self.idx += 1;
		Some(*entry)
	}
}

const_assert!(size_of::<DiskDirEntry>() == DIR_ENTRY_SIZE);

// Directory Entry Flag
/// Flag bit indicating a directory entry is in use.
///
/// When bit 0 of the `flags` field in `DiskDirEntry` is set, it indicates
/// the entry contains valid filename-to-inode mapping. Cleared entries are
/// available for reuse.
pub const DIRENT_USED: u16 = 1;

// Shouldn't the bitmap bits also hold which resource block they are pointing to?
// Nope the position of the bitmap is the pointer .. that's the whole point of it!
// A Bitmap is a view over raw bytes
/// Bitmap data structure for tracking allocation of inodes or data blocks.
///
/// Each bit in the underlying byte slice represents whether a resource (inode or data block)
/// is allocated (1) or free (0). The bit index directly corresponds to the resource index.
/// Provides methods to set, clear, and find free bits efficiently.
#[derive(Debug)]
#[repr(C)]
pub struct Bitmap<'a> {
	/// Mutable reference to the underlying byte array representing the bitmap.
	/// Each byte contains 8 bits, each tracking one resource.
	pub map: &'a mut [u8],
}
// used the ceiling function to calculate the minimum number of bytes required to store this

const_assert!(core::mem::size_of::<Bitmap>() <= BLOCK_SIZE);

impl<'a> Bitmap<'a> {
	/// creates a new bitmap overlaying a mutable byte slice
	///
	/// Creates a new Bitmap that wraps a mutable byte slice.
	///
	/// # Arguments
	/// * `map` - Mutable byte slice representing the bitmap
	///
	/// # Returns
	/// A new Bitmap instance that operates on the provided byte slice
	///
	/// # Note
	/// The byte slice is typically a 512-byte block buffer read from disk.
	pub fn new(map: &'a mut [u8]) -> Self {
		Self { map }
	}

	/// Takes the bit index not the byte index
	/// Checks if the bit at a given index is set to 1
	///
	/// Checks if a resource (inode or data block) is currently allocated.
	///
	/// # Arguments
	/// * `idx` - The bit index (resource number) to check
	///
	/// # Returns
	/// * `true` - If the bit is set (resource is allocated)
	/// * `false` - If the bit is clear (resource is free)
	///
	/// # Implementation
	/// Uses bit-masking to check individual bits:
	/// - `byte_index = idx / 8` - Finds which byte contains the bit
	/// - `bit_index = idx % 8` - Finds position within that byte
	/// - `(byte & (1 << bit_index)) != 0` - Tests if bit is set
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
	///
	/// Marks a resource (inode or data block) as allocated.
	///
	/// # Arguments
	/// * `idx` - The bit index (resource number) to set
	///
	/// # Returns
	/// * `Ok(())` - If the bit was successfully set
	/// * `Err(BitmapError::AlreadyAllocated)` - If the bit was already set
	///
	/// # Implementation
	/// Uses bitwise OR to set the bit: `byte |= (1 << bit_index)`
	///
	/// # Note
	/// This method enforces that bits can only be set once, preventing
	/// double allocation of resources.
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
	///
	/// Marks a resource (inode or data block) as free.
	///
	/// # Arguments
	/// * `idx` - The bit index (resource number) to clear
	///
	/// # Returns
	/// * `Ok(())` - If the bit was successfully cleared
	/// * `Err(BitmapError::AlreadyCleared)` - If the bit was already clear
	///
	/// # Implementation
	/// Uses bitwise AND with negation to clear the bit: `byte &= !(1 << bit_index)`
	///
	/// # Note
	/// This method enforces that bits can only be cleared once, preventing
	/// double freeing of resources.
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

	/// Finds the first free (unset) bit, sets it, and returns its index.
	///
	/// Scans the bitmap to find the first available resource, atomically
	/// allocates it, and returns its index. This is the primary allocation
	/// method used by the filesystem.
	///
	/// # Returns
	/// * `Some(usize)` - The index of the newly allocated resource
	/// * `None` - If no free resources are available (bitmap is full)
	///
	/// # Implementation
	/// Uses an optimized two-level scan:
	/// 1. **Byte-level scan**: Skips fully-allocated bytes (0xFF) for speed
	/// 2. **Bit-level scan**: Within each non-full byte, finds first clear bit
	/// 3. **Allocation**: Calls `set()` to mark the bit as allocated
	///
	/// # Performance
	/// - Best case: O(1) - First bit is free
	/// - Worst case: O(n) - Must scan entire bitmap
	/// - Average case: Much faster than naive bit-by-bit scan due to byte-level skip
	///
	/// # Note
	/// The caller should ensure the bitmap length matches the resource count
	/// to avoid allocating out-of-bounds indices.
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

/// Errors that can occur during bitmap operations.
///
/// These errors indicate invalid bitmap state transitions that would
/// violate resource allocation invariants.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(C)]
pub enum BitmapError {
	/// Attempted to allocate a resource that is already allocated.
	///
	/// Occurs when calling `set()` on a bit that is already 1.
	/// This prevents double allocation of the same inode or data block.
	AlreadyAllocated,
	/// Attempted to free a resource that is already free.
	///
	/// Occurs when calling `clear()` on a bit that is already 0.
	/// This prevents double freeing of resources.
	AlreadyCleared,
}

/// Type of filesystem entry (file, directory, etc.).
///
/// Stored in the inode to distinguish between different kinds of filesystem objects.
/// The u16 representation is stored on disk and must be convertible to/from this enum.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u16)]
pub enum FileType {
	/// Unknown or uninitialized type.
	///
	/// Should not normally appear in a valid filesystem, but provided
	/// as a safe default value.
	Unknown = 0,
	/// Regular file.
	///
	/// Contains arbitrary data, readable and writable.
	File = 0x1,
	/// Directory containing other files and directories.
	///
	/// Contains directory entries mapping names to inode numbers.
	Directory = 0x2,
}

/// Converts a u16 value to a FileType.
///
/// Used when deserializing inodes from disk to validate the file type field.
///
/// # Arguments
/// * `value` - The u16 value read from the disk inode
///
/// # Returns
/// * `Ok(FileType)` - If the value maps to a valid file type
/// * `Err(())` - If the value is not a recognized file type (corruption)
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

/// Converts a FileType to its u16 representation.
///
/// Used when serializing inodes to disk format.
impl From<FileType> for u16 {
	fn from(value: FileType) -> Self {
		value as u16
	}
}

// We need something to store the directories too .. some on-disk data structure is needed to
// store the directories too, so we'll reserve on one block for this that would hold the entire
// mapping for the filenames
