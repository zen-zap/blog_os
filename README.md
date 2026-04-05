# x86_64 OS

This is an OS written entirely in Rust.

This initially started as a follow of Philipp Oppermann's blog [Writing an OS](https://os.phil-opp.com/) in Rust.

After finishing the blog, I've tried my hand at taking this further.

I work on this on and off, so refinements and bug fixes always remain.

[I also wrote a blog about whatever I made here.](https://ashup.me/projects/creo-os) Pardon me if the blog is not updated with the latest code (will do it soon).

Own Contributions so far [this is updated]:

- Custom File System (includes VirtIO block device drivers)
- Interrupt Handling with ACPI
- Bitmap Frame Allocator

## Setup:

**Requirements:**

- [Rust](https://rustup.rs/) (Nightly toolchain)
- `llvm-tools-preview` component (`rustup component add llvm-tools-preview`)
- [QEMU](https://www.qemu.org/download/) (Specifically `qemu-system-x86_64`)

The files are stored in a disk.img file at project root. For creating a 64MiB raw file use:

```bash
dd if=/dev/zero of=disk.img bs=1M count=64
```

To compile the kernel, build the bootloader and launch the OS in QEMU:

```bash
cargo run --bin builder
```

### AI Usage:

As modules grow older, I have cleaned them up and added documentation using AI.
There was a lot of explanation comments that I wrote while first learning them. They were cluttering up the file. No major code was delegated to AI.
