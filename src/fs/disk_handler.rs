use crate::storage::block_repr::{BlockData, BlockDevice, BlockError};
use alloc::vec::Vec;

pub struct DiskImage {
	data: Vec<u8>,
	block_size: usize,
}

impl Default for DiskImage {
	fn default() -> Self {
		// standard sector size is 512
		Self { data: Vec::new(), block_size: 512 }
	}
}

impl DiskImage {
	/// creates a new empty DiskImage with 512 block_size
	fn new() -> Self {
		Default::default()
	}

	/// Loads the given data into a block
	pub fn load_from_memory(data: Vec<u8>) -> Self {
		DiskImage { data, block_size: 512 }
	}
}

impl BlockDevice for DiskImage {
	fn read_block(
		&mut self,
		block_id: u64,
		buffer: &mut [u8],
	) -> Result<(), BlockError> {
		let start = (block_id as usize) * (self.block_size);
		let end = start + self.block_size;

		if end > self.data.len() {
			return Err(BlockError::Read);
		}

		buffer.copy_from_slice(&self.data[start..end]);
		Ok(())
	}

	fn write_block(
		&mut self,
		block_id: u64,
		buffer: &[u8],
	) -> Result<(), BlockError> {
		let start = (block_id as u64) * (self.block_size as u64);
		let end = start + self.block_size as u64;

		if (buffer.len() as u64 > (end - start)) {
			return Err(BlockError::Write);
		}

		self.data[start as usize..end as usize].copy_from_slice(&buffer);

		Ok(())
	}

	fn block_size(&mut self) -> usize {
		self.block_size
	}

	fn block_count(&mut self) -> u64 {
		(self.data.len() / self.block_size) as u64
	}
}
