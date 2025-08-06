use alloc::vec::Vec;

/// Represents data held within a BlockDevice
#[derive(Debug)]
pub struct BlockData {
	// headers: Option<u8>, // add proper metadata support later
	data: Vec<u8>,
}

/// Represents the different Errors that can occur when dealing with BlockDevices
#[derive(Debug)]
pub enum BlockError {
	InvalidBlockId,
	Read,
	Write,
	InvalidDataStream,
}

/// Trait representing a block device capable of reading, writing, and querying block information.
///
/// Implementors of this trait provide mechanisms to:
/// - Read data from a specific block into a buffer.
/// - Write data from a buffer to a specific block.
/// - Query the size of each block.
/// - Query the total number of blocks available.
///
/// Errors during operations are reported via the `BlockError` enum.
pub trait BlockDevice {
	fn read_block(
		&mut self,
		block_id: u64,
		buffer: &mut [u8],
	) -> Result<(), BlockError>;
	fn write_block(
		&mut self,
		block_id: u64,
		buffer: &[u8],
	) -> Result<(), BlockError>;
	fn block_size(&mut self) -> usize;
	fn block_count(&mut self) -> u64;
}