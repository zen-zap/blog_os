/// helper module to interact with the VGA buffer. Unsafe is abstracted here.
// src/vga_buffer.rs

#[allow(dead_code)]

/// refer [here](https://os.phil-opp.com/vga-text-mode/#volatile)
use volatile::Volatile;

// gotta make some colors .. the bright bit combines with the normal bits to form the bright colors
// in the VGA buffer 
//
/// contains the different colors possible through the VGA buffer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]  // enable copy semantics and make it printable and comparable
#[repr(u8)] // each of the colors is represented by an 8-bit unsigned integer
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}


/// to represent a full color code that specifies the foreground and background color
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(transparent)]  // tells that it should have the exact same memory layout as its fields i.e. u8
struct ColorCode(u8);  // see the newtype idiom

impl ColorCode {
    fn new(foreground: Color, background: Color) -> ColorCode {
        // Higher 4 bits: background
        // Lower  4 bits: foreground
        ColorCode((background as u8) << 4 | (foreground as u8))
        // background is shifted by 4 bits .. placing it in the higher nibble
    }
}


/// to represent a screen character in the VGA text buffer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)] // so that the struct fields are laid out in order as in C .. not in rust by default
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

// the VGA text buffer is a 2D array that has 25 rows and 80 columns
/// the VGA screen displays 25 lines of text
const BUFFER_HEIGHT: usize = 25;
/// each VGA line can show 80 characters
const BUFFER_WIDTH: usize = 80;

/// to represent the VGA Buffer -- 2D array <br>
/// It is a contiguous block of memory starting at 0xb8000
#[repr(transparent)]
struct Buffer {
    /// 2D array to represent characters
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT], 
}

/// writer type to write into the screen [VGA]
pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    /// **mutable** reference to the buffer <br> -- reference passed since you **can't own/create** the VGA
    buffer: &'static mut Buffer, // guaranteed to be valid for the entire duration of the program
}


impl Writer {

    /// writes a bytes to the VGA buffer <br>
    /// parameters: <br>
    /// byte: u8  -- the byte you want to write -- 8 bits
    pub fn write_byte(&mut self, byte: u8)
    {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT -1;
                let col = self.column_position;

                let color_code = self.color_code;
                self.buffer.chars[row][col].write(ScreenChar {  // the compiler will never optimize this write
                    ascii_character: byte,
                    color_code: color_code,
                });
                self.column_position += 1; // since you wrote one byte move to the next column
            }
        }
    }

    /// prints a new line to the VGA buffer
    pub fn new_line(&mut self)
    {
        // move every character one line up and delete the upmost one

        for row in 1..BUFFER_HEIGHT
        {
            for col in 0..BUFFER_WIDTH
            {
                let character = self.buffer.chars[row][col].read();  // the read() method is provided by the Volatile type
                self.buffer.chars[row-1][col].write(character);
            }
        }

        self.clear_row(BUFFER_HEIGHT-1);
        self.column_position=0;
    }

    /// clears a raw by writing all of its characters with a space character
    fn clear_row(&mut self, row: usize)
    {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };

        for col in 0.. BUFFER_WIDTH
        {
            self.buffer.chars[row][col].write(blank);
            // the write method here is also provided by the Volatile type
        }
    }

    /// write a string into the VGA buffer <br>
    /// parameters: <br>
    /// s: &str
    ///
    /// <br> prints a '■' for unprintable bytes
    pub fn write_string(&mut self, s: &str)
    {
        for byte in s.bytes() {
            // rust strings are UTF-8 by default so they might contain some unsupported chars by
            // the vga_buffer
            match byte {
                // check for printable ascii_character or new_line
                // 0x20 is 32 in decimal for ' '
                // 0x7e is 126 in decimal for '~'
                // they denote the printable ASCII range
                // range inclusive notation -- remember it
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // not part of the of the printable ASCII range
                _ => self.write_byte(0xfe), // print a ■ for unprintable bytes 
            }
        }
    }
}

pub fn print_something() 
{
    use core::fmt::Write;

    let mut writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Blue),
        buffer: unsafe {
            &mut *(0xb8000 as *mut Buffer)
                // casting it into a raw mutable pointer and then derefencing through * and then
                // again getting a mutable pointer from that .. 
        },
    };

    writer.write_byte(b'H');
    writer.write_string("ello ");
    write!(writer, "The numbers are {} and {}", 42, 1.0/3.0).unwrap();
}


use core::fmt;

/// to support different formatting macros too!
/// gotta implement the core::fmt::Write trait -- used by write! and writeln!
impl fmt::Write for Writer {
    // gotta implement this ... for the trait to work!
    fn write_str(&mut self, s: &str) -> fmt::Result { // Result<(), fmt::Error>
        self.write_string(s);
        Ok(())
    }
}


use lazy_static::lazy_static;
use spin::Mutex;

lazy_static! {
    /// to create a global writer that can be used as an interface from other modules 
    /// without carrying a Writer instance around.. 
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Red),
        buffer: unsafe {
            &mut *(0xb8000 as *mut Buffer)
        },
    });
}
