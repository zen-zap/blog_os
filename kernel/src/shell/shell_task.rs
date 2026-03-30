//! in kernel/src/shell/shell_task.rs
//!
//! Asynchronous Shell Task
//!
//! Provides an interactive command-line interface for Creo OS.

#![allow(clippy::collapsible_match, clippy::collapsible_if)]

use crate::{GLOBAL_FS, print, println, shell::fs_com::Fsc, task::keyboard};
use alloc::{string::String, vec::Vec};
use pc_keyboard::{DecodedKey, HandleControl, KeyCode, Keyboard, ScancodeSet1, layouts};

pub async fn run_shell_task() {
	let mut line_buffer = String::with_capacity(256);
	let mut scancodes = keyboard::ScancodeStream::new();
	let mut keyboard = Keyboard::new(ScancodeSet1::new(), layouts::Us104Key, HandleControl::Ignore);

	println!("Welcome to Creo OS Shell");
	println!("Type 'help' for a list of commands.");

	loop {
		print!("creo ~ $ ");
		line_buffer.clear();

		loop {
			match keyboard::read_key(&mut scancodes, &mut keyboard).await {
				DecodedKey::RawKey(key) => match key {
					KeyCode::Return => {
						println!();
						process_command(&line_buffer).await;
						break;
					},
					KeyCode::Backspace => {
						if !line_buffer.is_empty() {
							line_buffer.pop();
							print!("\u{8} \u{8}");
						}
					},
					_ => {},
				},
				DecodedKey::Unicode(character) => {
					if character == '\n' || character == '\r' {
						println!();
						process_command(&line_buffer).await;
						break;
					} else if character == '\u{8}' || character == '\u{7F}' {
						if !line_buffer.is_empty() {
							line_buffer.pop();
							print!("\u{8} \u{8}");
						}
					} else if character.is_ascii_graphic() || character == ' ' {
						line_buffer.push(character);
						print!("{}", character);
					}
				},
			}
		}
	}
}

async fn process_command(line: &str) {
	let line = line.trim();
	if line.is_empty() {
		return;
	}

	let mut parts = line.split_whitespace();
	let command = parts.next().unwrap_or("");
	let args: Vec<&str> = parts.collect();

	let fsc = Fsc::new();

	match command {
		"echo" => {
			println!("{}", args.join(" "));
		},
		"whoami" => {
			println!("zen-zap");
		},
		"clear" => {
			for _ in 0..100 {
				println!();
			}
		},
		"shutdown" | "exit" => {
			println!("shutting down...");
			crate::exit_qemu(crate::QemuExitCode::Success);
		},
		"panic" => {
			panic!("user-initiated kernel panic via shell!");
		},
		"help" => {
			println!("Creo OS Shell v0.2");
			println!("System Commands:");
			println!("  echo <text>    - Print text to the screen");
			println!("  whoami         - Print the current user");
			println!("  clear          - Clear the terminal screen");
			println!("  panic          - Trigger a deliberate kernel panic");
			println!("  shutdown/exit  - Halt the OS and exit QEMU");
			println!("\nFile System Commands:");
			println!("  ls [path]      - List directory contents (default: /)");
			println!("  touch <path>   - Create an empty file");
			println!("  mkdir <path>   - Create a new directory");
			println!("  cat <path>     - Print file contents");
			println!("  rm <path>      - Delete a file");
			println!("  write <path>   - Write following arguments to a file");
		},

		"ls" | "touch" | "mkdir" | "rm" | "cat" | "write" => {
			if let Ok(fs_mutex) = GLOBAL_FS.try_get() {
				let path = args.first().unwrap_or(&"/");

				match command {
					"ls" => fsc.ls(path, fs_mutex),
					"touch" => fsc.touch(path, fs_mutex),
					"mkdir" => fsc.mkdir(path, fs_mutex),
					"rm" => fsc.rm(path, fs_mutex),
					"cat" => fsc.cat(path, fs_mutex),
					"write" => {
						let lines = if args.len() > 1 {
							&args[1..]
						} else {
							&[]
						};
						fsc.write(path, lines, fs_mutex);
					},
					_ => unreachable!(),
				}
			} else {
				crate::error!("File System is not initialized!");
			}
		},

		_ => {
			println!("Unknown command: '{}'. Type 'help' for commands.", command);
		},
	}
}
