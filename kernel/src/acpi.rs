use core::ptr::NonNull;

use acpi::{AcpiError, Handler, PhysicalMapping};

#[derive(Clone, Copy)]
pub struct AcpiHandler {
	physical_memory_offset: u64,
}

impl AcpiHandler {
	pub fn new(physical_memory_offset: u64) -> Self {
		Self { physical_memory_offset }
	}
}

/*
 * Something from the docs itself:
 *
 * An implementation of this `Handler` trait must be provided to allow acpi to perform operations that interface with the underlying hardware and other systems in your host implementation.
 * This interface is designed to be flexible to allow usage of the library from a variety of settings.
 * Depending on your usage of this library, not all functionality may be required. If you do not provide certain functionality,
 * you should return AcpiError::HostUnimplemented.
 *
 * The library will attempt to propagate this error back to the host if an operation cannot be performed without that functionality.
 *
 * The Handler must be cheaply clonable (e.g. a reference, Arc, marker struct, etc.) as a copy of the handler is stored in various structures,
 * such as in each PhysicalMapping to facilitate unmapping.
 * */

impl Handler for AcpiHandler {
	unsafe fn map_physical_region<T>(
		&self,
		physical_address: usize,
		size: usize,
	) -> acpi::PhysicalMapping<Self, T> {
		let virtual_address = physical_address as u64 + self.physical_memory_offset;

		PhysicalMapping {
			physical_start: physical_address,
			virtual_start: NonNull::new(virtual_address as *mut T).unwrap(),
			region_length: size,
			mapped_length: size,
			handler: self.clone(),
		}
	}

	fn unmap_physical_region<T>(region: &acpi::PhysicalMapping<Self, T>) {
		// we don't do anything here since the bootloader memory mapping is permanent
	}

	fn read_u8(
		&self,
		address: usize,
	) -> u8 {
		unsafe {
			core::ptr::read_volatile((address as u64 + self.physical_memory_offset) as *const u8)
		}
	}
	fn read_u16(
		&self,
		address: usize,
	) -> u16 {
		unsafe {
			core::ptr::read_volatile((address as u64 + self.physical_memory_offset) as *const u16)
		}
	}
	fn read_u32(
		&self,
		address: usize,
	) -> u32 {
		unsafe {
			core::ptr::read_volatile((address as u64 + self.physical_memory_offset) as *const u32)
		}
	}
	fn read_u64(
		&self,
		address: usize,
	) -> u64 {
		unsafe {
			core::ptr::read_volatile((address as u64 + self.physical_memory_offset) as *const u64)
		}
	}

	/*
	 * The following things are unimplemented.
	 *
	 * ACPI - Advanced Configuration and Power Interface
	 *
	 * When the OS wants to know the battery percentage, turn on a cooling fan, or change the CPU clock speed,
	 * it doesn't talk to the hardware directly.
	 *
	 * The motherboard manufacturers write actual code (compiled into a bytecode called AML) and store it in the ACPI tables.
	 *
	 * To do these dynamic things, we need an AML interpreter which executes this firmware code.
	 * There is no use for this now.
	 *
	 * PCI Config Space Routing:
	 *
	 * ACPI helps the OS figure out how these PCIe slots are wired to the CPU's interrupt lines.
	 * No need for this yet! There is a basic PCI scanner to find the VirtIO disk.
	 *
	 *
	 * Sleep States:
	 *
	 * ACPI is responsible for power management. ACPI manages the transition from
	 * S3 (suspend to ram) to S4 (hibernate/suspend to disk). No need for this as well.
	 *
	 *
	 * TLDR; Just put the MADT in the bag bruh.
	 *
	 * */

	fn write_u8(
		&self,
		_address: usize,
		_value: u8,
	) {
		unimplemented!()
	}
	fn write_u16(
		&self,
		_address: usize,
		_value: u16,
	) {
		unimplemented!()
	}
	fn write_u32(
		&self,
		_address: usize,
		_value: u32,
	) {
		unimplemented!()
	}
	fn write_u64(
		&self,
		_address: usize,
		_value: u64,
	) {
		unimplemented!()
	}
	fn read_io_u8(
		&self,
		_port: u16,
	) -> u8 {
		unimplemented!()
	}
	fn read_io_u16(
		&self,
		_port: u16,
	) -> u16 {
		unimplemented!()
	}
	fn read_io_u32(
		&self,
		_port: u16,
	) -> u32 {
		unimplemented!()
	}
	fn write_io_u8(
		&self,
		_port: u16,
		_value: u8,
	) {
		unimplemented!()
	}
	fn write_io_u16(
		&self,
		_port: u16,
		_value: u16,
	) {
		unimplemented!()
	}
	fn write_io_u32(
		&self,
		_port: u16,
		_value: u32,
	) {
		unimplemented!()
	}
	fn read_pci_u8(
		&self,
		_address: acpi::PciAddress,
		_offset: u16,
	) -> u8 {
		unimplemented!()
	}
	fn read_pci_u16(
		&self,
		_address: acpi::PciAddress,
		_offset: u16,
	) -> u16 {
		unimplemented!()
	}
	fn read_pci_u32(
		&self,
		_address: acpi::PciAddress,
		_offset: u16,
	) -> u32 {
		unimplemented!()
	}
	fn write_pci_u8(
		&self,
		_address: acpi::PciAddress,
		_offset: u16,
		_value: u8,
	) {
		unimplemented!()
	}
	fn write_pci_u16(
		&self,
		_address: acpi::PciAddress,
		_offset: u16,
		_value: u16,
	) {
		unimplemented!()
	}
	fn write_pci_u32(
		&self,
		_address: acpi::PciAddress,
		_offset: u16,
		_value: u32,
	) {
		unimplemented!()
	}
	fn nanos_since_boot(&self) -> u64 {
		unimplemented!()
	}
	fn stall(
		&self,
		_microseconds: u64,
	) {
		unimplemented!()
	}
	fn sleep(
		&self,
		_milliseconds: u64,
	) {
		unimplemented!()
	}
	fn create_mutex(&self) -> acpi::Handle {
		unimplemented!()
	}
	fn acquire(
		&self,
		_mutex: acpi::Handle,
		_timeout: u16,
	) -> Result<(), acpi::aml::AmlError> {
		unimplemented!()
	}
	fn release(
		&self,
		_mutex: acpi::Handle,
	) {
		unimplemented!()
	}
}
