//! in src/shell/shell_task.rs

#![allow(clippy::collapsible_match, clippy::collapsible_if)]

use super::fs_com::Fsc;
use crate::{GLOBAL_FS, print, println, task::keyboard};
use alloc::string::String;
use alloc::vec::Vec;
use pc_keyboard::{DecodedKey, HandleControl, KeyCode, Keyboard, ScancodeSet1, layouts};

pub async fn run_shell_task() {
	let mut line_buffer = String::with_capacity(256);
	let mut scancodes = keyboard::ScancodeStream::new();
	let mut keyboard = Keyboard::new(
		ScancodeSet1::new(),
		layouts::Us104Key,
		HandleControl::Ignore, // we'll handle special keys ourselves
	);
	loop {
		// prompt
		print!("shell >>>  ");
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
						},
						KeyCode::Backspace => {
							if !line_buffer.is_empty() {
								line_buffer.pop();
								print!("\u{8} \u{8}");
							}
						},
						_ => {},
					}
				},
				DecodedKey::Unicode(character) => {
					// Handle Enter as Unicode character too
					if character == '\n' || character == '\r' {
						println!(); // Move to the next line
						process_command(&line_buffer).await;
						break; // Exit inner loop to print new prompt
					}
					// Handle Backspace as ASCII character (0x08 or 0x7F)
					else if character == '\u{8}' || character == '\u{7F}' {
						if !line_buffer.is_empty() {
							line_buffer.pop();
							print!("\u{8} \u{8}");
						}
					}
					// Only printable characters
					if character.is_ascii_graphic() || character == ' ' {
						line_buffer.push(character);
						print!("{}", character);
					}
				},
			}
		}
	}
}

async fn process_command(line: &str) {
	if !line.is_empty() {
		println!("COMMAND: {}", line);
	} else {
		println!("EMPTY COMMAND! TYPE SOMETHING!");
		return; // do nothing
	}

	// TODO:
	// - parse them into command and argument
	// - get access to the file system
	// - write up basic commands like ls, echo, touch, cat
	// - cd, mkdir, rmdir will be added later once we have proper directory support/path traversal
	// - split up basic commands and file system commands into separate modules and call those
	// function here

	let mut parts = line.split_whitespace();
	let command = parts.next().unwrap_or("");
	let args: Vec<&str> = parts.collect();

	let mut fsc = Fsc::new();

	if let Ok(fs_mutex) = GLOBAL_FS.try_get() {
		match command {
			"ls" => {
				let path: &str = args.first().unwrap_or(&"/");
				fsc.ls(path, fs_mutex);
			},

			"touch" => {
				let path: &str = args.first().unwrap_or(&"/");
				fsc.touch(path, fs_mutex);
			},

			"mkdir" => {
				let path: &str = args.first().unwrap_or(&"/");
				fsc.mkdir(path, fs_mutex);
			},

			"rm" => {
				let path: &str = args.first().unwrap_or(&"/");
				fsc.rm(path, fs_mutex);
			},

			"cat" => {
				let path: &str = args.first().unwrap_or(&"/");
				fsc.cat(path, fs_mutex);
			},

			"write" => {
				let path: &str = args.first().unwrap_or(&"/");
				let lines: Vec<&str> = args[1..].to_vec();
				fsc.write(path, &lines, fs_mutex);
			},

			"help" => {
				println!("SFS Shell v0.1");
				println!("Available commands:");
				println!("  ls [path]      - List directory contents (default: /)");
				println!("  touch <path>   - Create an empty file");
				println!("  mkdir <path>   - Create a new directory");
				println!("  cat <path>     - Print file contents");
				println!("  rm <path>      - Delete a file");
				println!("  write <path> .. - Write content to a file (overwrite)");
				println!("  help           - Show this message");
			},

			_ => {
				println!("Unknown command: '{}'. Type 'help' for commands.", command);
			},
		}
	}
}
