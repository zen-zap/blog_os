use std::path::PathBuf;
use std::process::Command;

const BOOT_UEFI: bool = true;

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

	println!("Creating UEFI disk image...");
	let uefi_path = PathBuf::from("target/uefi.img");
	bootloader::UefiBoot::new(&kernel_path).create_disk_image(&uefi_path).unwrap();

	println!("Booting creo OS...");
	let mut cmd = Command::new("qemu-system-x86_64");

	if BOOT_UEFI {
		println!("Booting in UEFI mode");
		// uefi boot requires the ovmf firmware
		cmd.arg("-bios").arg("OVMF.fd");
		cmd.arg("-drive").arg(format!("format=raw,file={}", uefi_path.display()));
	} else {
		println!("Booting in BIOS mode");
		cmd.arg("-drive").arg(format!("format=raw,file={}", bios_path.display()));
	}
	cmd.arg("-drive").arg("file=disk.img,format=raw,if=none,id=disk0");
	cmd.arg("-device").arg("virtio-blk-pci,drive=disk0");
	cmd.arg("-device").arg("isa-debug-exit,iobase=0xf4,iosize=0x04");
	cmd.arg("-m").arg("256M");

	// headless aware for CI
	if std::env::var("CI").is_ok() {
		cmd.arg("-display").arg("none");
	} else {
		cmd.arg("-display").arg("gtk,zoom-to-fit=on");
		cmd.arg("-vga").arg("std");
		// this tells qemu to emulate a Bochs VBE compatible graphics card
		// in UEFI, OVMF detects the vga hardware and uses the GOP to init the framebuffer
		cmd.arg("-global").arg("VGA.xres=1280");
		cmd.arg("-global").arg("VGA.yres=800");
	}

	// pipe to serial port
	cmd.arg("-serial").arg("stdio");

	let mut child = cmd.spawn().expect("Failed to launch QEMU");
	child.wait().unwrap();
}
