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
struct Selectors {
	code_selector: SegmentSelector,
	data_selector: SegmentSelector,
	tss_selector: SegmentSelector,
}

use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable};

lazy_static! {
	static ref GDT: (GlobalDescriptorTable, Selectors) = {
		let mut gdt = GlobalDescriptorTable::new();

		let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
		let data_selector = gdt.add_entry(Descriptor::kernel_data_segment());
		let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));

		(gdt, Selectors { code_selector, data_selector, tss_selector })
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
		CS::set_reg(GDT.1.code_selector);
		DS::set_reg(GDT.1.data_selector);
		ES::set_reg(GDT.1.data_selector);
		SS::set_reg(GDT.1.data_selector);

		load_tss(GDT.1.tss_selector);
	}
}
