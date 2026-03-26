This follows Philipp Oppermann's blog [Writing an OS](https://os.phil-opp.com/) in Rust.

After finishing the blog, I've made a simple file system here.

Refinements and bug fixes remain.

[You can read about the project here](https://ashup.me/projects/blog-os)

The files are stored in a disk.img file. For creating a 64MiB raw file use:

`dd if=/dev/zero of=disk.img bs=1M count=64`