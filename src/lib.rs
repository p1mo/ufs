//! Embed a directory at compile time, or walk one when the program runs.
//!
//! Both sources show up as [`Entry`] values. Embedded bytes are borrowed from
//! the binary; on-disk bytes are read on demand into an owned buffer.
//! `content()` never leaks memory and never panics because a file is missing.
//!
//! There are no third-party dependencies. Optional compression is the built-in
//! ULZ1 codec, selected per embed with `compress = true`.

#![forbid(unsafe_code)]

mod compress;
mod embeds;
mod entry;
mod error;
mod registry;
mod statics;

pub use compress::{MAX_UNCOMPRESSED, compress, decompress};
pub use embeds::Embeds;
pub use entry::{Entry, MAX_DISK_READ};
pub use error::Error;
pub use registry::{Bind, UFS};
pub use statics::{Statics, Walker};

#[doc(hidden)]
pub use ufs_macros::bind_dir_impl;

/// Embed every regular file under a directory into the binary.
///
/// The path is relative to `CARGO_MANIFEST_DIR` unless `absolute = true`.
/// Relative paths are not allowed to escape the crate root; pass
/// `absolute = true` to opt into a directory outside it.
///
/// | Option | Default | Meaning |
/// | --- | --- | --- |
/// | `read` | `true` | Embed bytes. `false` records path and size only. |
/// | `root` | `false` | Prefix stored paths with the directory's own name. |
/// | `absolute` | `false` | Use `path` as given instead of joining it to the manifest dir. |
/// | `compress` | `false` | Store ULZ1 when it is smaller than the raw file. Requires `read = true`. |
///
/// Symlinks are skipped. The walk refuses the Cargo target directory, the
/// crate root itself (unless `absolute`), more than 100_000 files, and files
/// bigger than 64 MiB when bytes are embedded.
///
/// ```
/// use ufs::bind_dir;
///
/// let assets = bind_dir!("examples/files");
/// let demo = assets.get("demo.txt").unwrap();
/// assert!(demo.content().unwrap().starts_with(b"Lorem ipsum"));
/// assert!(!demo.is_local());
/// ```
#[macro_export]
macro_rules! bind_dir {
    ($($tt:tt)*) => {
        $crate::bind_dir_impl! { @crate $crate; $($tt)* }
    };
}
