// in src/virtio/pci

use crate::println;
use virtio_drivers::transport::pci::bus::{ConfigurationAccess, DeviceFunction, PciRoot};
use x86_64::instructions::port::Port;

const CONFIG_ADDRESS: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

/// Reads a 32-bit value from the PCI configuration space.
unsafe fn read_config_dword(
	bus: u8,
	device: u8,
	function: u8,
	offset: u8,
) -> u32 {
	let mut address_port = Port::new(CONFIG_ADDRESS);
	let mut data_port = Port::new(CONFIG_DATA);

	// Construct the address packet
	let address = (bus as u32) << 16
		| (device as u32) << 11
		| (function as u32) << 8
		| (offset as u32 & 0xFC) // align to 4 bytes
		| 0x80000000; // Enable bit

	address_port.write(address);
	data_port.read()
}

/// Scans the PCI bus for a VirtIO device using the correct `enumerate_bus` method.
pub fn scan(root: &PciRoot<PciConfigIo>) -> Option<DeviceFunction> {
	// Takes &PciRoot, not &mut
	println!("[PCI] Scanning for devices...");

	// We must manually iterate through all possible buses.
	for bus_num in 0..=255 {
		// The `enumerate_bus` method gives us an iterator for all devices on a specific bus.
		for (device_func, header) in root.enumerate_bus(bus_num) {
			println!(
				"  - Found device on bus {}, device {} -> Vendor={:?}, Device={:?}",
				bus_num, device_func.device, header.vendor_id, header.device_id
			);

			// Check for a VirtIO device (Vendor ID 0x1AF4)
			if header.vendor_id == 0x1AF4 {
				println!("    -> Found a VirtIO device!");

				// We found it. We'll assume it's on function 0.
				// You could add logic here to check other functions if needed.
				let device_function =
					DeviceFunction { bus: bus_num, device: device_func.device, function: 0 };

				return Some(device_function);
			}
		}
	}

	// If we get here, no VirtIO device was found on any bus.
	None
}

// In src/pci.rs

/// An implementation of `ConfigurationAccess` that uses x86 I/O ports to access the
/// PCI configuration space.
#[derive(Debug, Copy, Clone)]
pub struct PciConfigIo;

impl ConfigurationAccess for PciConfigIo {
	fn read_word(
		&self,
		device_function: DeviceFunction,
		register_offset: u8,
	) -> u32 {
		let mut address_port = Port::new(0xCF8);
		let mut data_port = Port::new(0xCFC);

		let DeviceFunction { bus, device, function } = device_function;

		// Construct the address packet
		let address = (bus as u32) << 16
			| (device as u32) << 11
			| (function as u32) << 8
			| (register_offset as u32 & 0xFC) // align to 4 bytes
			| 0x80000000; // Enable bit

		unsafe {
			address_port.write(address);
			data_port.read()
		}
	}

	fn write_word(
		&mut self,
		device_function: DeviceFunction,
		register_offset: u8,
		data: u32,
	) {
		let mut address_port = Port::new(0xCF8);
		let mut data_port = Port::new(0xCFC);

		let DeviceFunction { bus, device, function } = device_function;

		// Construct the address packet
		let address = (bus as u32) << 16
			| (device as u32) << 11
			| (function as u32) << 8
			| (register_offset as u32 & 0xFC) // align to 4 bytes
			| 0x80000000; // Enable bit

		unsafe {
			address_port.write(address);
			data_port.write(data);
		}
	}

	unsafe fn unsafe_clone(&self) -> Self {
		PciConfigIo
	}
}
