use crate::{acpi::AcpiHandler, error, info};
use acpi::{
	AcpiTables,
	platform::{AcpiPlatform, InterruptModel},
};

/// Parses ACPI tables, disables the legacy PIC, and initializes the modern APIC & I/O APIC.
pub fn init(
	rsdp_addr: u64,
	phy_offset_val: u64,
) {
	info!("Initializing Hardware Interrupt Controllers...");

	let acpi_handler = AcpiHandler::new(phy_offset_val);
	let acpi_tables = unsafe { AcpiTables::from_rsdp(acpi_handler, rsdp_addr as usize) }
		.expect("Failed to parse ACPI tables");

	if let Ok(platform_info) = AcpiPlatform::new(acpi_tables, acpi_handler) {
		if let InterruptModel::Apic(apic_info) = platform_info.interrupt_model {
			let local_apic_phys_addr = apic_info.local_apic_address;
			info!("  - Local APIC physical address: {:#x}", local_apic_phys_addr);

			unsafe {
				use x86_64::instructions::port::Port;
				let mut pic1_data: Port<u8> = Port::new(0x21);
				let mut pic2_data: Port<u8> = Port::new(0xA1);
				pic1_data.write(0xFF);
				pic2_data.write(0xFF);
			}

			let local_apic_vaddr = local_apic_phys_addr + phy_offset_val;
			let sivr_ptr = (local_apic_vaddr + 0xF0) as *mut u32;

			unsafe {
				let mut sivr = core::ptr::read_volatile(sivr_ptr);
				sivr |= 0x100 | 0xFF;
				core::ptr::write_volatile(sivr_ptr, sivr);
			}

			if let Some(io_apic) = apic_info.io_apics.first() {
				let io_apic_phys_addr = io_apic.address;
				info!("  - Found I/O APIC at physical address: {:#x}", io_apic_phys_addr);

				// io apic uses indirect memory to save space
				// it only exposes 2 address to the CPU

				let io_apic_vaddr = io_apic_phys_addr as u64 + phy_offset_val;
				let ioregsel = io_apic_vaddr as *mut u32; // the index register (0x00)
				let iowin = (io_apic_vaddr + 0x10) as *mut u32; // data register (0x10)

				unsafe {
					// keyboard is IRQ_1
					let irq1_lower_index = 0x12;
					let vector = 33;
					core::ptr::write_volatile(ioregsel, irq1_lower_index);
					core::ptr::write_volatile(iowin, vector);
				}
			} else {
				error!("No I/O APIC found in the ACPI tables!");
			}
		} else {
			error!("System Interrupt Model is not APIC!");
		}
	} else {
		error!("Failed to parse AcpiPlatform from ACPI tables");
	}
}
