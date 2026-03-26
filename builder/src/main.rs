use std::path::PathBuf;
use std::process::Command;

fn main() {
	println!("Building creo OS kernel...");

	let status = Command::new("cargo")
		.arg("build")
		.arg("--package")
		.arg("creo")
		.arg("--target")
		.arg("x86_64-unknown-none")
		.status()
		.expect("Failed to run cargo build");

	if !status.success() {
		panic!("Kernel build failed!");
	}

	let kernel_path = PathBuf::from("target/x86_64-unknown-none/debug/creo");

	if !kernel_path.exists() {
		panic!("Could not find kernel binary at: {}", kernel_path.display());
	}

	println!("Creating BIOS disk image...");
	let bios_path = PathBuf::from("target/bios.img");

	bootloader::BiosBoot::new(&kernel_path).create_disk_image(&bios_path).unwrap();

	println!("Booting creo OS...");
	let mut cmd = Command::new("qemu-system-x86_64");

	cmd.arg("-drive").arg(format!("format=raw,file={}", bios_path.display()));
	cmd.arg("-drive").arg("file=disk.img,format=raw,if=none,id=disk0");
	cmd.arg("-device").arg("virtio-blk-pci,drive=disk0");
	cmd.arg("-m").arg("256M");
	cmd.arg("-display").arg("gtk,zoom-to-fit=on");
	cmd.arg("-vga").arg("std");
	cmd.arg("-global").arg("VGA.xres=1280");
	cmd.arg("-global").arg("VGA.yres=800");

	// pipe to serial port
	cmd.arg("-serial").arg("stdio");

	let mut child = cmd.spawn().expect("Failed to launch QEMU");
	child.wait().unwrap();
}
