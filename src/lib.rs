//! Embed a directory at compile time, or walk one when the program runs.
//!
//! Both sources show up as [`Entry`] values inside one [`UFS`] collection.
//! `bind_dir!`, [`UFS::statics`], [`UFS::embedded`], [`UFS::bytes`], and
//! [`UFS::archive`] all return that type. Embedded bytes are borrowed from
//! the binary; on-disk bytes are read on demand. `content()` borrows embedded
//! and owned bytes, allocates only when it has to decompress or read disk, and
//! never panics because a file is missing. `bind_dir!` uses `include_bytes!`
//! for source files. Compressed bytes are a static literal, not a side cache.
//!
//! Optional ULZ1 compression is built in (`compress = fast`, `balanced`, or
//! `max`). `content()` decompresses any level. Zip, tar, tar.gz, tar.xz, and
//! 7z live in this crate: [`UFS::archive`] and [`Entry::archive`] open them,
//! and `archives = true` expands them while embedding.

#![forbid(unsafe_code)]

mod archive;
mod compress;
mod entry;
mod error;
mod set;
mod statics;

pub use archive::{Node, expand, is_archive_bytes, is_archive_name};
pub use compress::{Level, MAX_UNCOMPRESSED, compress, compress_many, compress_with, decompress};
pub use entry::{Entry, MAX_DISK_READ};
pub use error::Error;
pub use set::UFS;
pub use statics::{Statics, Walker};

#[doc(hidden)]
pub use ufs_macros::bind_dir_impl;

/// Embed every regular file under a directory into the binary.
///
/// The path is the only required argument. It is relative to `CARGO_MANIFEST_DIR`
/// unless `absolute = true`. `..` is allowed, so a path may leave the crate
/// directory. `root = true` stores that directory's own name on each entry
/// (`../vendor/assets` becomes `assets/file.txt`).
///
/// Every named option is optional.
///
/// | Option | Default | Meaning |
/// | --- | --- | --- |
/// | `read` | `true` | Embed bytes. `false` records path and size only. |
/// | `root` | `false` | Prefix stored paths with the directory's own name. |
/// | `absolute` | `false` | Use `path` as given instead of joining it to the manifest dir. |
/// | `compress` | `false` | ULZ1 level: `false`/`off`/`none`, `fast`/`min`, `true`/`balanced`/`default`, `max`/`best`/`high`. A string (`compress = "max"`) is also accepted. Requires `read = true`. `content()` decompresses any level automatically. |
/// | `archives` | `false` | Expand zip, tar, tar.gz, tar.xz, and 7z into entries. Requires `read = true`. |
/// | `archive_entry` | `true` | With `archives`, also keep each archive file as an entry. |
///
/// Symlinks are skipped. The walk still refuses the filesystem root, the crate
/// root itself (unless `absolute`), and the Cargo target directory. Embeds stop
/// at 100,000 files, 64 MiB each, and 256 MiB total.
///
/// ```
/// use ufs::bind_dir;
///
/// let assets = bind_dir!("examples/files");
/// let demo = assets.get("demo.txt").unwrap();
/// assert!(demo.content().unwrap().starts_with(b"Lorem ipsum"));
/// assert!(!demo.is_local());
///
/// let rooted = bind_dir!("examples/../examples/files", root = true, compress = fast);
/// let again = rooted.get("files/demo.txt").unwrap();
/// assert!(again.content().unwrap().starts_with(b"Lorem ipsum"));
/// ```
#[macro_export]
macro_rules! bind_dir {
    ($($tt:tt)*) => {
        $crate::bind_dir_impl! { @crate $crate; $($tt)* }
    };
}
