use bootloader_api::info::{FrameBuffer, FrameBufferInfo, PixelFormat};
use core::fmt::{self, Write};
use spin::Mutex;

/// This struct holds a static mutable reference to the framebuffer alongwith
/// information and coordinate positions.
#[derive(Debug)]
pub struct GraphicsWriter {
	framebuffer: &'static mut [u8],
	info: FrameBufferInfo,
	x_pos: usize,
	y_pos: usize,
}

/// Global graphics writer wrapped in a Mutex
pub static GRAPHICS_WRITER: Mutex<Option<GraphicsWriter>> = Mutex::new(None);
// do I need an async mutex here? Nope
// we'll use it to print debug messages everywhere
// we cannot .await inside an interrupt
// because it is not an async task

// why is the initial value None here? because when we start the OS
// and it runs into a panic
// without the framebuffer yet to be initialized
// there would be nothing to print to
// so it would just be None

impl GraphicsWriter {
	pub fn new(fb: &'static mut FrameBuffer) -> Self {
		let info = fb.info();
		GraphicsWriter { framebuffer: fb.buffer_mut(), info, x_pos: 0, y_pos: 0 }
	}

	fn set_pixel(
		&mut self,
		x: usize,
		y: usize,
		r: u8,
		g: u8,
		b: u8,
	) {
		// so we have a 2D grid of pixels
		// memory is linear not 2D
		// so we have to translate
		// framebuffer provides us width and stride
		// width would be the width of the screen .. say 1920
		// stride is the number of pixels between the start of the line and the start of the next
		// (stride defn. as per docs)
		// for the sake of memory alignment (reduces cpu cycles to read data)
		// we often need the bytes to be aligned to 16 or 32 something
		// stride is returned by the framebuffer from bootinfo as per your hardware
		// so it is padded properly, hence we use stride
		let offset = (y * self.info.stride + x) * self.info.bytes_per_pixel;

		if offset + 2 < self.framebuffer.len() {
			// offset + 2 since we writing 3 separate bytes into the 1D array of framebuffer
			match self.info.pixel_format {
				PixelFormat::Rgb => {
					self.framebuffer[offset] = r;
					self.framebuffer[offset + 1] = g;
					self.framebuffer[offset + 2] = b;
				},
				PixelFormat::Bgr => {
					self.framebuffer[offset] = b;
					self.framebuffer[offset + 1] = g;
					self.framebuffer[offset + 2] = r;
				},
				PixelFormat::U8 => {
					// Grayscale fallback
					let gray = (r as u32 + g as u32 + b as u32) / 3;
					self.framebuffer[offset] = gray as u8;
				},
				_ => {},
			}
		}
	}

	fn write_string(
		&mut self,
		s: &str,
	) {
		todo!();
	}
}

impl fmt::Write for GraphicsWriter {
	fn write_str(
		&mut self,
		s: &str,
	) -> fmt::Result {
		self.write_string(s);
		Ok(())
	}
}

/// print functionality from the framebuffer implementation
pub fn _print(args: fmt::Arguments) {
	use x86_64::instructions::interrupts;

	interrupts::without_interrupts(|| {
		// temporarily output to serial for debug
		crate::serial::_print(args);

		// pass these arguments to graphics writer??
		// use the write_str implementation we just did
		// it provides us write_fmt which takes the arguments
		// breaks them down into string slices and feeds them into
		// the write_str function
		if let Some(mut writer_lock) = GRAPHICS_WRITER.try_lock() {
			if let Some(writer) = writer_lock.as_mut() {
				writer.write_fmt(args).expect("printing to framebuffer failed");
			}
		}
	});
}
