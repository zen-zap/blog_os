//! in src/shell/shell_task.rs

use crate::{println, print, task::keyboard};
use alloc::string::String;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, KeyCode, ScancodeSet1};

pub async fn run_shell_task() {
	let mut line_buffer = String::with_capacity(256);
	let mut scancodes = keyboard::ScancodeStream::new();
	let mut keyboard = Keyboard::new(
		ScancodeSet1::new(),
		layouts::Us104Key,
		HandleControl::Ignore // we'll handle special keys ourselves
	);
	loop {
		// prompt
		print!("> ");
		// clear buffer
		line_buffer.clear();
		// read lines
		loop {
			match keyboard::read_key(&mut scancodes, &mut keyboard).await {
				DecodedKey::RawKey(key) => {
					match key {
						KeyCode::Return => {
                        println!(); // Move to the next line
                        process_command(&line_buffer).await;
                        break; // Exit inner loop to print new prompt
                    	}
						KeyCode::Backspace => {
							if !line_buffer.is_empty() {
								line_buffer.pop();
								print!("\u{8}"); // Move cursor back one
							}
						}
						_ => {},
					}
				}
				DecodedKey::Unicode(character) => {
					if character.is_ascii_graphic() || character == ' ' {
						line_buffer.push(character);
						print!("{}", character);
					}
				}
			}
		}
	}
}

async fn process_command(line: &str) {
	if !line.is_empty() {
		println!("COMMAND: {}", line);
	}

	// TODO:
	// - parse them into command and argument
	// - get access to the file system
	// - write up basic commands like ls, echo, touch, cat
	// - cd, mkdir, rmdir will be added later once we have proper directory support/path traversal

}