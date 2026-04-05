use crate::fs::simple_fs::{FileSystem, GLOBALFSType, SFS};
use crate::virtio::OsHal;
use crate::virtio::pci;
use crate::virtio::pci::PciConfigIo;
use crate::{GLOBAL_FS, error, fs_debug, hlt_loop, info, warn};
use no_std_async::Mutex as AsyncMutex;
use spin::Mutex;
use virtio_drivers::{
	device::blk::VirtIOBlk,
	transport::pci::{PciTransport, bus::PciRoot},
};

const RUN_FILESYSTEM_SELF_CHECK: bool = false;

pub fn init_filesystem() {
	let pci_config_access = PciConfigIo;
	let mut pci_root = PciRoot::new(pci_config_access);

	if let Some(device_function) = pci::scan(&mut pci_root) {
		let mut pci_root_mut = pci_root;
		let transport = PciTransport::new::<OsHal, _>(&mut pci_root_mut, device_function)
			.expect("Failed to create PCI transport");

		let mut blk_dev =
			VirtIOBlk::<OsHal, _>::new(transport).expect("failed to create blk driver");

		let mut fs = match SFS::mount(blk_dev) {
			Ok(fs) => fs,
			Err(_) => {
				warn!("Mount failed or filesystem not found! Formatting disk...");

				// We need to re-create the block device
				let mut pci_root_for_format = PciRoot::new(pci_config_access);
				let transport =
					PciTransport::new::<OsHal, _>(&mut pci_root_for_format, device_function)
						.expect("Failed to re-create transport for format");

				let blk_dev_for_format = VirtIOBlk::<OsHal, _>::new(transport)
					.expect("Failed to re-create blk_dev for format");

				let mut fs = SFS::format(blk_dev_for_format).expect("Failed to format disk.");

				fs.init_root_directory().expect("Failed to init root directory");

				fs
			},
		};

		if RUN_FILESYSTEM_SELF_CHECK {
			run_filesystem_self_check(&mut fs);
		}

		GLOBAL_FS
			.try_init_once(|| AsyncMutex::new(fs))
			.expect("Failed to initialize GLOBAL_FS");
	} else {
		error!("No VirtIO block device found!");
	}
}

fn run_filesystem_self_check(fs: &mut GLOBALFSType) {
	info!("--- Starting SFS C-W-R-D Test ---");
	let filename = "test.txt";

	fs_debug!("Attempting cleanup of '{}'...", filename);
	match fs.delete_file(filename) {
		Ok(_) => {
			fs_debug!("Cleanup successful.");
		},
		Err(_) => {
			fs_debug!("File not present, no cleanup needed.");
		},
	}

	fs_debug!("Creating '{}'...", filename);
	let handle = match fs.create_file(filename) {
		Ok(h) => {
			info!("File created successfully with handle {:?}", h);
			h
		},
		Err(e) => {
			error!("Failed to create file: {:?}", e);
			hlt_loop();
		},
	};

	let lines_to_write = &["Hello from your SFS!", "This is the second line."];
	fs_debug!("Writing content to file...");
	match fs.write_file_lines(handle, lines_to_write) {
		Ok(_) => info!("Content written successfully."),
		Err(e) => {
			error!("Failed to write to file: {:?}", e);
			hlt_loop();
		},
	}

	fs_debug!("Reading content back from file...");
	match fs.read_file(handle) {
		Ok(content) => {
			info!("Read success! Content:\n---\n{}\n---", content);

			let expected_content = lines_to_write.join("\n");
			if content == expected_content {
				info!("Verification SUCCESS: Content matches!");
			} else {
				error!("Verification FAILED: Content mismatch!");
			}
		},
		Err(e) => {
			error!("Failed to read from file: {:?}", e);
			hlt_loop();
		},
	}

	fs_debug!("Listing root directory...");
	match fs.list_file(".") {
		Ok(files) => info!("Files in root: {:?}", files),
		Err(e) => error!("Failed to list files: {:?}", e),
	}

	fs_debug!("Deleting '{}'...", filename);
	match fs.delete_file(filename) {
		Ok(_) => info!("File deleted successfully."),
		Err(e) => error!("Failed to delete file: {:?}", e),
	}

	fs_debug!("Listing root directory after delete...");
	match fs.list_file(".") {
		Ok(files) => info!("Files in root: {:?}", files),
		Err(e) => error!("Failed to list files: {:?}", e),
	}

	info!("--- SFS Test Complete ---");
}
