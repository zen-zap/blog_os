use uart_16550::SerialPort;
use spin::Mutex;
use lazy_static::lazy_static;

lazy_static! // init method called exactly once on its first use 
{
    pub static ref SERIAL1: Mutex<SerialPort> = {

        let mut serial_port = unsafe {
            SerialPort::new(0x3F8)  // standard port number for the first serial interface
        };

        serial_port.init();
        Mutex::new(serial_port)
    };
}

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        SERIAL1.lock().write_fmt(args).expect("Printing to Serial failed!");
    });

    // disbaling interrupts shouldn't be the general solution .. it increases the worst-case
    // interrupt latency 
}

// using macro_export makes it live directly under the crate root .. so crate::serial::serial_println will not work

/// prints to the host through the serial interface
#[macro_export]
macro_rules! serial_print {

    ($($arg: tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    };
}

/// prints to the host through the serial interface, appending a newline
#[macro_export]
macro_rules! serial_println {

    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(concat!($fmt, "\n"), $($arg)*));
}

// SerialPort type already implements the fmt::Write trait
