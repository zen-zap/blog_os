//! Tracing and logging macros for blog_os
//!
//! This module provides structured logging macros that can be conditionally
//! compiled based on feature flags. Perfect for OS development where traditional
//! debuggers don't work well.

/// General tracing macro - lowest level, very verbose
#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        #[cfg(feature = "trace-all")]
        {
            $crate::print!("[TRACE] ");
            $crate::println!($($arg)*);
        }
    };
}

/// Debug level logging - detailed information for debugging
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        #[cfg(any(feature = "trace-all", feature = "debug-fs", feature = "debug-virtio", feature = "debug-memory"))]
        {
            $crate::print!("[DEBUG] ");
            $crate::println!($($arg)*);
        }
    };
}

/// Info level logging - general information (always compiled)
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        {
            $crate::print!("[INFO]  ");
            $crate::println!($($arg)*);
        }
    };
}

/// Warning level logging - potentially problematic situations
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        {
            $crate::print!("[WARN]  ");
            $crate::println!($($arg)*);
        }
    };
}

/// Error level logging - error conditions
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        {
            $crate::print!("[ERROR] ");
            $crate::println!($($arg)*);
        }
    };
}

/// Filesystem-specific debug logging
#[macro_export]
macro_rules! fs_debug {
    ($($arg:tt)*) => {
        #[cfg(any(feature = "trace-all", feature = "debug-fs"))]
        {
            $crate::print!("[FS-DEBUG] ");
            $crate::println!($($arg)*);
        }
    };
}

/// VirtIO-specific debug logging
#[macro_export]
macro_rules! virtio_debug {
    ($($arg:tt)*) => {
        #[cfg(any(feature = "trace-all", feature = "debug-virtio"))]
        {
            $crate::print!("[VIRTIO-DEBUG] ");
            $crate::println!($($arg)*);
        }
    };
}

/// Memory management debug logging
#[macro_export]
macro_rules! memory_debug {
    ($($arg:tt)*) => {
        #[cfg(any(feature = "trace-all", feature = "debug-memory"))]
        {
            $crate::print!("[MEM-DEBUG] ");
            $crate::println!($($arg)*);
        }
    };
}

/// PCI/Hardware debug logging
#[macro_export]
macro_rules! pci_debug {
    ($($arg:tt)*) => {
        #[cfg(any(feature = "trace-all", feature = "debug-hardware"))]
        {
            $crate::print!("[PCI-DEBUG] ");
            $crate::println!($($arg)*);
        }
    };
}

/// Function entry tracing - shows when functions are called
#[macro_export]
macro_rules! trace_function {
	() => {
		#[cfg(feature = "trace-all")]
		{
			$crate::println!("[TRACE] Entering: {}", core::any::type_name::<fn()>());
		}
	};
	($func_name:expr) => {
		#[cfg(feature = "trace-all")]
		{
			$crate::println!("[TRACE] Entering: {}", $func_name);
		}
	};
}

/// Execution point tracing - shows exact location in code
#[macro_export]
macro_rules! trace_here {
	() => {
		#[cfg(feature = "trace-all")]
		{
			$crate::println!("[TRACE] {}:{}", file!(), line!());
		}
	};
	($msg:expr) => {
		#[cfg(feature = "trace-all")]
		{
			$crate::println!("[TRACE] {}:{} - {}", file!(), line!(), $msg);
		}
	};
}

/// Conditional debug macro that takes a feature flag
#[macro_export]
macro_rules! debug_if {
    ($feature:literal, $($arg:tt)*) => {
        #[cfg(feature = $feature)]
        {
            $crate::print!("[DEBUG-");
            $crate::print!($feature);
            $crate::print!("] ");
            $crate::println!($($arg)*);
        }
    };
}

/// Panic with context - shows file and line where panic occurred
#[macro_export]
macro_rules! panic_here {
    ($($arg:tt)*) => {
        panic!("{}:{} - {}", file!(), line!(), format_args!($($arg)*))
    };
}
