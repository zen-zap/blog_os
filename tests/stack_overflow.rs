#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;
use blog_os::serial_print;

#[no_mangle]
pub extern "C" fn _start() -> !
{
    serial_print!("stack_overflow::stack_overflow...\t");

    blog_os::gdt::init();

    // make a custom double fault handler that does an exit_qemu(QemuExitCode::Success) instead of panicking
    init_test_idt();

    stack_overflow();

    panic!("Execution continued after stack_overflow");
}

#[panic_handler]
fn panic(info: &PanicInfo) -> !
{
    blog_os::test_panic_handler(info)
}

#[allow(unconditional_recursion)]
fn stack_overflow()
{
    stack_overflow(); // for each recursion the return address is pushed
    volatile::Volatile::new(0).read(); // prevent tail recursion optims 
                                       // prevents the tail call elimination
}


use lazy_static::lazy_static;
use x86_64::structures::idt::InterruptDescriptorTable;

lazy_static! {

    static ref TEST_IDT: InterruptDescriptorTable = {

        let mut idt = InterruptDescriptorTable::new();

        unsafe {
            idt.double_fault
                .set_handler_fn(test_double_fault_handler)
                .set_stack_index(blog_os::gdt::DOUBLE_FAULT_IST_INDEX);
        }

        idt
    };
}

pub fn init_test_idt()
{
    TEST_IDT.load();
}


use blog_os::{exit_qemu, QemuExitCode, serial_println};
use x86_64::structures::idt::InterruptStackFrame;

extern "x86-interrupt" fn test_double_fault_handler(_stack_frame: InterruptStackFrame, _error_code: u64) -> !
{
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop{}
}
