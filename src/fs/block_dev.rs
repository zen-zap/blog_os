use super::simple_fs::FileSystemError;
use crate::println;
use crate::virtio::OsHal;
use virtio_drivers::{device::blk::VirtIOBlk, transport::pci::PciTransport};

/// Interface to any storage that presents itself in fixed-size-blocks
///
/// Implementors of this trait provide mechanisms to:
/// - Read data from a specific block into a buffer.
/// - Write data from a buffer to a specific block.
/// - Query the capacity.
///
/// Errors during operations are reported via the `BlockError` enum.
pub trait BlockDevice {
	/// reads one of more blocks starting from a block_id into the buffer
	fn read_blocks(
		&mut self,
		block_id: u64,
		buffer: &mut [u8],
	) -> Result<(), FileSystemError>;
	/// writes one or more blocks from a buffer into the device
	fn write_blocks(
		&mut self,
		block_id: u64,
		buffer: &[u8],
	) -> Result<(), FileSystemError>;
	/// returns the total number of blocks on the device
	fn capacity(&self) -> usize;
}

/// Represents the different Errors that can occur when dealing with BlockDevices
#[derive(Debug)]
pub enum BlockError {
	InvalidBlockId,
	Read,
	Write,
	InvalidDataStream,
}

impl BlockDevice for VirtIOBlk<OsHal, PciTransport> {
	fn read_blocks(
		&mut self,
		start_block_id: u64,
		buffer: &mut [u8],
	) -> Result<(), FileSystemError> {
		self.read_blocks(start_block_id as usize, buffer).map_err(|e| {
			println!("[BLOCK DEVICE] Read Error: {}", e);
			FileSystemError::BlockError
		})
	}

	fn write_blocks(
		&mut self,
		start_block_id: u64,
		buffer: &[u8],
	) -> Result<(), FileSystemError> {
		self.write_blocks(start_block_id as usize, buffer).map_err(|e| {
			println!("[BLOCK DEVICE] Write Error: {}", e);
			FileSystemError::BlockError
		})
	}

	fn capacity(&self) -> usize {
		self.capacity() as usize
	}
}
