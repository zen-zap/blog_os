// src/gdt.rs
//
// creates a dedicated stack for handling double faults

use lazy_static::lazy_static;
use x86_64::VirtAddr; // represents a virtual address in the memory
use x86_64::structures::tss::TaskStateSegment;

/// indicates which entry in the IST array will be used as a dedicated stack for handling double faults
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
	/// A TSS is a data structure used by x86_64 CPUs to store information about a task’s state. <br>
	/// One of its key roles is to hold an Interrupt Stack Table (IST), which is an array of stack pointers. <br>
	/// These pointers are used to switch to known-good stacks when handling critical exceptions—like double faults.
	///
	/// The TSS in-turn is stored within the GDT
	static ref TSS: TaskStateSegment = {

		let mut tss = TaskStateSegment::new();

		// we assign a stack pointer here to the defined index
		// assigns the top of the DOUBLE FAULT STACK to the appropriate IST entry in the TSS
		tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {

			/// define the STACK SIZE
			const STACK_SIZE: usize = 4096 * 5; // defines a stack of 5 pages

			/// define the STACK
			static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

			// stacks on x86 grow downwards .. i.e. from higher addresses to lower addresses

			// calc virtual address of the start of the array
			let stack_start = VirtAddr::from_ptr(&raw const STACK); // raw pointers are not subject to borrowship rules
			let stack_end = stack_start + STACK_SIZE; // initial stack pointer ... top of the stack

			stack_end // write this pointer for the double fault handler
		};

		tss
	};
}

use x86_64::structures::gdt::SegmentSelector;

#[derive(Debug)]
struct Selectors {
	code_selector: SegmentSelector,
	tss_selector: SegmentSelector,
}

use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable};

lazy_static! {
	// data structure that defines the memory segments.
	// <br> each entry is called a Descriptor.
	// <br> This thing holds the TSS -- for your usage
	static ref GDT: (GlobalDescriptorTable, Selectors) = {
		let mut gdt = GlobalDescriptorTable::new();

		let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
		// check out what the kernle_code_segment entails .. it's some useful stuff

		let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
		// add the TSS you created to the newly created GDT

		(gdt, Selectors{
			code_selector,
			tss_selector
		})
	};
}

pub fn init() {
	use x86_64::instructions::segmentation::{CS, Segment};
	use x86_64::instructions::tables::load_tss;

	GDT.0.load(); // loads the GDT in 'static form

	unsafe {
		// the old code segment register might be pointing to a different GDT
		CS::set_reg(GDT.1.code_selector); // reload the code segment register

		// tell the CPU to use this TSS .. we loaded a GDT that contains a TSS selector
		load_tss(GDT.1.tss_selector); // load the TSS
	}
}
