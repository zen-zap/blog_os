// src/gdt.rs
//
// creates a dedicated stack for handling double faults

use lazy_static::lazy_static;
use x86_64::VirtAddr;
use x86_64::structures::tss::TaskStateSegment;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
	static ref TSS: TaskStateSegment = {
		let mut tss = TaskStateSegment::new();

		tss.privilege_stack_table[0] = {
			const STACK_SIZE: usize = 4096 * 5;
			static mut RSP0_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

			let stack_start = VirtAddr::from_ptr(&raw const RSP0_STACK);
			let stack_end = stack_start + STACK_SIZE;

			// return stack_end since the CPU uses addresses from high to low
			stack_end
		};

		tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
			const STACK_SIZE: usize = 4096 * 5;
			static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

			let stack_start = VirtAddr::from_ptr(&raw const STACK);
			let stack_end = stack_start + STACK_SIZE;

			stack_end
		};

		tss
	};
}

use x86_64::structures::gdt::SegmentSelector;

#[derive(Debug)]
pub struct Selectors {
	pub kernel_code_selector: SegmentSelector,
	pub kernel_data_selector: SegmentSelector,
	pub user_code_selector: SegmentSelector,
	pub user_data_selector: SegmentSelector,
	pub tss_selector: SegmentSelector,
}

use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable};

lazy_static! {
	// defines memory segments and privilege rings
	pub static ref GDT: (GlobalDescriptorTable, Selectors) = {
		let mut gdt = GlobalDescriptorTable::new();

		// ring 0 segments
		let kernel_code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
		let kernel_data_selector = gdt.add_entry(Descriptor::kernel_data_segment());
		// ring 3 segments
		let user_code_selector = gdt.add_entry(Descriptor::user_code_segment());
		let user_data_selector = gdt.add_entry(Descriptor::user_data_segment());

		let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));

		(gdt, Selectors { kernel_code_selector, kernel_data_selector, user_code_selector, user_data_selector, tss_selector })
	};
}

pub fn init() {
	use x86_64::instructions::segmentation::{CS, DS, ES, SS, Segment};
	use x86_64::instructions::tables::load_tss;

	GDT.0.load();
	// this executes lgdt (Load Global Descriptor Table)
	// this simply points the GDTR (special CPU register) to the memory address of the new table

	unsafe {
		// reload all the segment registers so that we get rid of the hidden cache
		// only loading kernel selectors here
		// we are in ring 0, so loading user selectors would crash us
		CS::set_reg(GDT.1.kernel_code_selector);
		DS::set_reg(GDT.1.kernel_data_selector);
		ES::set_reg(GDT.1.kernel_data_selector);
		SS::set_reg(GDT.1.kernel_data_selector);

		load_tss(GDT.1.tss_selector);
	}
}
