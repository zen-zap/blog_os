//! in kernel/src/interrupts.rs
//!
//! CPU Exception and Hardware Interrupt Management
//!
//! This module configures the Interrupt Descriptor Table (IDT). The IDT tells the CPU
//! which Rust functions to execute when hardware interrupts (like a keyboard press)
//! or CPU exceptions (like a Page Fault) occur.
//!
//! We currently route hardware interrupts through the modern Advanced Programmable
//! Interrupt Controller (APIC), completely bypassing the legacy 8259 PIC.

use crate::gdt;
use crate::hlt_loop;
use crate::virtio::PHYSICAL_MEMORY_OFFSET;
use crate::{debug, error};
use lazy_static::lazy_static;
use x86_64::registers::control::Cr2;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

/// Hardware Interrupt Vectors mapped to the APIC.
/// We start at 32 to avoid colliding with the 0-31 CPU exceptions.
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
	Timer = 32,
	Keyboard = 33,
}

impl InterruptIndex {
	fn as_u8(self) -> u8 {
		self as u8
	}

	fn as_usize(self) -> usize {
		usize::from(self.as_u8())
	}
}

lazy_static! {
	/// The global IDT. It must have a 'static lifetime because the CPU
	/// will reference it on every interrupt
	static ref IDT: InterruptDescriptorTable = {
		let mut idt = InterruptDescriptorTable::new();
		idt.breakpoint.set_handler_fn(breakpoint_handler);
		idt.page_fault.set_handler_fn(page_fault_handler);

		unsafe{
			idt.double_fault.set_handler_fn(double_fault_handler)
				.set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
		}

		idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer_interrupt_handler);
		idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(keyboard_interrupt_handler);

		idt
	};
}

/// Initializes the Interrupt Descriptor Table.
///
/// Loads the global IDT into the CPU.
pub fn init_idt() {
	IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
	debug!("EXCEPTION: BREAKPOINT\n {:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
	stack_frame: InterruptStackFrame,
	_error_code: u64,
) -> ! {
	error!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
	loop {
		core::hint::spin_loop();
	}
}

extern "x86-interrupt" fn page_fault_handler(
	stack_frame: InterruptStackFrame,
	error_code: PageFaultErrorCode,
) {
	error!("EXCEPTION: PAGE FAULT");
	error!("Accessed Address at CR2 Register: {:?}", Cr2::read());
	error!("Error Code: {:?}", error_code);
	error!("{:#?}", stack_frame);
	hlt_loop();
}

/// Signals the Local APIC that the current interrupt has been fully processed.
/// Without this, the APIC will block all future interrupts.
fn notify_apic_eoi() {
	unsafe {
		let phys_offset = PHYSICAL_MEMORY_OFFSET;

		let local_apic_vaddr = 0xfee00000 + phys_offset;
		// The EOI register is located at offset 0xB0 from the APIC base.
		let eoi_ptr = (local_apic_vaddr + 0xB0) as *mut u32;

		core::ptr::write_volatile(eoi_ptr, 0);
	}
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
	notify_apic_eoi();
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
	use x86_64::instructions::port::Port;
	let mut port = Port::new(0x60);
	let scancode: u8 = unsafe { port.read() };
	crate::task::keyboard::add_scancode(scancode);

	notify_apic_eoi();
}
