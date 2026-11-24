<div align="center">

# ufs

**Small library for reading and embeding directorys.**

[![Crates.io](https://img.shields.io/crates/v/ufs?style=for-the-badge)](https://crates.io/crates/ufs)
[![Documentation](https://img.shields.io/docsrs/ufs?style=for-the-badge)](https://docs.rs/ufs)
[![License](https://img.shields.io/crates/l/ufs?style=for-the-badge)](https://crates.io/crates/ufs)

</div>

Embed a directory into a Rust binary at compile time, or walk a directory on disk at runtime. Both sources use the same `UFS` collection and `Entry` interface, with optional ULZ1 compression and archive expansion.

## Features

- **Compile-time directory embedding:** `bind_dir!` discovers files while compiling and emits references to their contents with `include_bytes!`.
- **Runtime directory walking:** `Statics` provides lazy iteration or collection of files from a real directory.
- **One collection type:** embedded files, disk files, archive members, static byte slices, and owned buffers are represented by `UFS` and `Entry`.
- **Optional ULZ1 compression:** select `fast`, `balanced`, or `max` when embedding; `Entry::content()` transparently decompresses entries.
- **Archive support:** inspect or expand ZIP, TAR, TAR.GZ, TAR.XZ, and 7z files, including nested archives.
- **Bounded and defensive reads:** directory traversal skips symlinks, archive paths are sanitized, and file and expansion limits are enforced.

## Requirements

- Rust 1.85 or newer.

The crate uses Rust 2024 edition. The `ufs` package manifest declares `rust-version = "1.85"`.

## Installation

Add `ufs` to the consuming crate's `Cargo.toml`:

```toml
[dependencies]
ufs = "0.1"
```

The `bind_dir!` macro is re-exported by `ufs`; applications do not need to depend on `ufs-macros` directly.

## Quick start: embed a directory

`bind_dir!` embeds all regular files below a directory. Its path is resolved relative to the consuming crate's `CARGO_MANIFEST_DIR` by default.

```rust
use ufs::bind_dir;

fn main() -> Result<(), ufs::Error> {
    let assets = bind_dir!("examples/files", compress = true);

    let demo = assets.get("demo.txt").expect("demo.txt is embedded");
    let contents = demo.content()?;

    assert!(contents.starts_with(b"Lorem ipsum"));
    println!("{} bytes", contents.len());
    Ok(())
}
```

The example path above exists in this repository. In an application, replace it with a directory inside that application's crate. Embedding happens during compilation; the original files do not need to be present at runtime.

## Embedding options

Only the directory path is required. Every named option is optional and may appear after the path:

```rust
use ufs::bind_dir;

let assets = bind_dir!(
    "assets",
    root = true,
    compress = fast,
    archives = true,
    archive_entry = true,
);
```

| Option | Default | Description |
| --- | --- | --- |
| `read` | `true` | Embed file contents. With `false`, entries retain their path and size but `content()` returns `Error::MissingContent`. |
| `root` | `false` | Prefix each embedded path with the embedded directory's own name. For example, `assets/logo.svg` instead of `logo.svg`. |
| `absolute` | `false` | Treat the supplied path as absolute. When enabled, the path must actually be absolute. Otherwise, it is joined to `CARGO_MANIFEST_DIR`. |
| `compress` | `false` | Compress embedded contents using ULZ1. Accepted values are listed below. Requires `read = true`. |
| `archives` | `false` | Expand supported archives into entries at compile time. Requires `read = true`. |
| `archive_entry` | `true` | When `archives = true`, keep the archive file as an entry alongside its members. |

`compress` accepts these values (bare identifiers or string values where shown):

| Value | Meaning |
| --- | --- |
| `false`, `off`, `none` | Do not compress. |
| `true`, `balanced`, `default` | Balanced compression effort. `true` is the default compression level. |
| `fast`, `min` | Search less for matches. |
| `max`, `best`, `high` | Search more extensively for matches. |

For example:

```rust
let plain = bind_dir!("assets", compress = false);
let balanced = bind_dir!("assets", compress = true);
let smaller = bind_dir!("assets", compress = "max");
let names_only = bind_dir!("assets", read = false);
let rooted = bind_dir!("assets", root = true);
```

`Entry::content()` returns the original, uncompressed bytes regardless of the selected compression level. The level controls the encoder's search effort, not the decoder format.

### Embedded path and traversal rules

- Symlinks are skipped.
- The filesystem root and the Cargo target directory cannot be embedded.
- The crate root itself is refused unless `absolute = true` is used.
- The default path is relative to `CARGO_MANIFEST_DIR`; `..` path components are allowed.
- `root = true` adds the selected directory's final name to each stored path.
- Embedded paths use `/` separators and are sorted by stored path during macro expansion.

Compile-time embedding is limited to 100,000 files, 64 MiB per file, and 256 MiB total embedded input. Directory nesting is limited to 64 levels.

## Runtime directory walking

Use `Statics` to inspect files that exist on disk when the program runs. The root must be an existing real directory, not a symlink.

```rust
use ufs::{Error, Statics};

fn main() -> Result<(), Error> {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/files");

    for item in Statics::new(root).walk()? {
        let entry = item?;
        let bytes = entry.content()?;
        println!("{} — {} bytes", entry.path.display(), bytes.len());
    }

    Ok(())
}
```

`Statics::walk()` returns a lazy depth-first `Walker`. Each iterator item is a `Result<Entry, Error>`, so a filesystem error is reported as an item rather than causing a panic. Directories are not returned as entries.

For a collected, sorted result, use `Statics::files()` or `UFS::statics()`:

```rust
use ufs::{Error, UFS};

fn main() -> Result<(), Error> {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/files");
    let files: UFS = UFS::statics(root)?;

    for entry in &files {
        println!("{} ({} bytes)", entry.path.display(), entry.size);
    }
    Ok(())
}
```

Runtime disk entries have absolute paths. `Statics::files()` and `UFS::statics()` return entries sorted by absolute path. The plain runtime walk does not expand archives unless archive expansion is explicitly enabled.

## Archives

Supported formats are ZIP, TAR, TAR.GZ (`.tar.gz` and `.tgz`), TAR.XZ (`.tar.xz` and `.txz`), and 7z. Archives are detected from their filename or content, depending on the API used.

### Expand archives while embedding

```rust
use ufs::bind_dir;

let files = bind_dir!("examples/archives", archives = true);
let hello = files
    .get("pack.zip/hello.txt")
    .expect("archive member is embedded");
assert_eq!(hello.content().unwrap().as_ref(), b"hello-zip");
```

With the default `archive_entry = true`, the archive file is kept alongside its members. Set `archive_entry = false` to include only the expanded members:

```rust
let members_only = bind_dir!(
    "examples/archives",
    archives = true,
    archive_entry = false,
);
assert!(members_only.get("pack.zip").is_none());
assert!(members_only.get("pack.zip/hello.txt").is_some());
```

Archive member paths are prefixed with the archive filename, such as `pack.zip/hello.txt`. Nested archives are expanded up to eight levels.

### Expand archives at runtime

```rust
use ufs::{Error, Statics};

fn main() -> Result<(), Error> {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/archives");
    let files = Statics::new(root)
        .archives(true)
        .archive_entry(false)
        .files()?;

    for entry in files {
        println!("{}", entry.path.display());
    }
    Ok(())
}
```

Alternatively, collect a directory into `UFS` and call `expand_archives`:

```rust
use ufs::{Error, UFS};

fn main() -> Result<(), Error> {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/archives");
    let mut files = UFS::statics(root)?;
    files.expand_archives(true)?; // keep each archive entry as well as its members
    Ok(())
}
```

`Entry::archive()` expands one archive and returns its members; it does not include the top-level archive entry. `UFS::archive()` returns a collection containing both the archive file and its expanded members.

Archive expansion enforces a 64 MiB limit per archive and per member, a 256 MiB total expanded-size limit, a 100,000-member limit, and an eight-level nesting limit. Directory entries and non-regular TAR entries (including symlinks and hard links) are not materialized. Unsafe member paths, including paths containing `..` or NUL, are rejected.

## Using one `UFS` type

`UFS` can contain entries from compile-time embedding, runtime walks, static byte slices, owned buffers, and archives. This makes it possible to pass different sources through the same application interfaces.

```rust
use ufs::{Error, UFS, bind_dir};

fn main() -> Result<(), Error> {
    let embedded: UFS = bind_dir!("examples/files");
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/files");
    let on_disk: UFS = UFS::statics(root)?;
    let note: UFS = UFS::embedded("note.txt", b"hello");
    let scratch: UFS = UFS::bytes("scratch.txt", b"owned".to_vec());
    let archive: UFS = UFS::archive(
        "pack.zip",
        include_bytes!("examples/archives/pack.zip"),
    )?;

    let mut all = UFS::new();
    all.append(embedded);
    all.append(on_disk);
    all.append(note);
    all.append(scratch);
    all.append(archive);

    for entry in &all {
        println!("{}: {} bytes", entry.path.display(), entry.content()?.len());
    }
    Ok(())
}
```

`UFS::read_all()` reads all entries in collection order. Embedded and owned buffers can be borrowed; disk reads and decompression allocate owned byte buffers. Disk reads and decompression are performed in parallel where there is more than one such entry.

## Public API reference

The items below are re-exported from the `ufs` crate root. The procedural macro implementation entry point `bind_dir_impl` is doc-hidden and is not intended as a consumer API; use `bind_dir!` instead.

### `bind_dir!`

```rust
bind_dir!($path:literal, /* optional named options */)
```

Embeds the regular files below `$path` at compile time. See [Embedding options](#embedding-options) for the supported options, defaults, and limits. The macro expands to a `UFS` value.

### `UFS`

`UFS` is the collection type for entries. It implements `Default`, `Clone`, `IntoIterator` for both owned and borrowed collections, and `Extend<Entry>`.

| Method | Signature | Description |
| --- | --- | --- |
| `new` | `UFS::new() -> UFS` | Create an empty collection. |
| `from_entries` | `UFS::from_entries(entries: Vec<Entry>) -> UFS` | Build a collection, preserving the supplied order. |
| `embedded` | `UFS::embedded(path: &str, bytes: &'static [u8]) -> UFS` | Create a collection with one static, uncompressed entry. |
| `entry` | `UFS::entry(path: &str, bytes: &'static [u8]) -> UFS` | Alias for `embedded`. |
| `bytes` | `UFS::bytes(path: &str, bytes: Vec<u8>) -> UFS` | Create a collection with one owned buffer. |
| `archive` | `UFS::archive(path: &str, bytes: &'static [u8]) -> Result<UFS, Error>` | Keep one static archive entry and expand its members, including nested archives. |
| `statics` | `UFS::statics(path: impl AsRef<Path>) -> Result<UFS, Error>` | Walk a directory at runtime and collect entries. Archives remain unopened. |
| `r#static` | `UFS::r#static(path: impl AsRef<Path>) -> Result<UFS, Error>` | Raw-identifier alias for `statics`. |
| `len` | `fn len(&self) -> usize` | Return the number of entries. |
| `is_empty` | `fn is_empty(&self) -> bool` | Check whether the collection has no entries. |
| `iter` | `fn iter(&self) -> std::slice::Iter<'_, Entry>` | Iterate over borrowed entries in collection order. |
| `get` | `fn get(&self, path: impl AsRef<Path>) -> Option<&Entry>` | Find the first entry whose stored path equals `path`. |
| `append` | `fn append(&mut self, other: UFS)` | Move entries from `other` onto the end of this collection. |
| `read_all` | `fn read_all(&self) -> Result<Vec<Cow<'_, [u8]>>, Error>` | Read every entry in order. |
| `expand_archives` | `fn expand_archives(&mut self, keep: bool) -> Result<(), Error>` | Expand archive entries in place, optionally retaining each archive entry. |

The collection can be iterated directly:

```rust
for entry in &files {
    println!("{}", entry.path.display());
}

for entry in files {
    println!("{}", entry.filename);
}
```

### `Entry`

`Entry` represents one file. Its public fields are:

| Field | Type | Meaning |
| --- | --- | --- |
| `size` | `u64` | Uncompressed size captured when the entry was discovered or constructed. |
| `depth` | `usize` | Number of parent directories in its stored path. A top-level file has depth `0`. |
| `path` | `PathBuf` | Relative for embedded/constructed entries; absolute for runtime disk entries. |
| `filename` | `String` | Final path component. |

Constructors and methods:

| Method | Signature | Description |
| --- | --- | --- |
| `embedded` | `Entry::embedded(relative: &str, bytes: &'static [u8]) -> Entry` | Construct an uncompressed entry backed by static bytes. |
| `compressed` | `Entry::compressed(relative: &str, raw_size: u64, bytes: &'static [u8]) -> Entry` | Construct an entry backed by a ULZ1 payload and its uncompressed size. |
| `omitted` | `Entry::omitted(relative: &str, size: u64) -> Entry` | Construct a path-and-size-only entry; reading it returns `Error::MissingContent`. |
| `bytes` | `Entry::bytes(relative: &str, bytes: Vec<u8>) -> Entry` | Construct an entry backed by an owned byte vector. |
| `is_archive` | `fn is_archive(&self) -> bool` | Check the name or available payload for a supported archive. |
| `archive` | `fn archive(&self) -> Result<UFS, Error>` | Expand this entry as an archive; the returned collection contains members, not the archive entry itself. |
| `is_local` | `fn is_local(&self) -> bool` | Return `true` for entries read from disk. |
| `is_compressed` | `fn is_compressed(&self) -> bool` | Return `true` for ULZ1-backed entries. |
| `is_absolute` | `fn is_absolute(&self) -> bool` | Check whether `path` is absolute. |
| `metadata` | `fn metadata(&self) -> Result<std::fs::Metadata, Error>` | Read filesystem metadata for a disk entry; embedded entries return `Error::NotOnDisk`. |
| `content` | `fn content(&self) -> Result<Cow<'_, [u8]>, Error>` | Return the file bytes, transparently decompressing ULZ1 data and reading disk-backed files on demand. |

Uncompressed static and owned bytes are returned as `Cow::Borrowed`. Compressed entries and disk entries return `Cow::Owned`. An omitted entry returns `Error::MissingContent`.

`MAX_DISK_READ` is also exported from the crate root and is set to `512 * 1024 * 1024` bytes (512 MiB). `Entry::content()` refuses larger disk files.

### `Statics` and `Walker`

`Statics` describes a directory to inspect at runtime. Its methods are:

| Method | Signature | Description |
| --- | --- | --- |
| `new` | `Statics::new(path: impl AsRef<Path>) -> Statics` | Create a walker configuration for `path`. |
| `archives` | `fn archives(self, enabled: bool) -> Self` | Enable or disable archive expansion during the walk. |
| `archive_entry` | `fn archive_entry(self, keep: bool) -> Self` | When archive expansion is enabled, choose whether to keep archive entries as well as their members. Defaults to `true`. |
| `path` | `fn path(&self) -> &Path` | Return the path passed to `new`, before canonicalization. |
| `walk` | `fn walk(&self) -> Result<Walker, Error>` | Start a lazy depth-first walk. |
| `load` | `fn load(&self) -> Result<UFS, Error>` | Collect the walk into a `UFS`. |
| `files` | `fn files(&self) -> Result<Vec<Entry>, Error>` | Collect and sort the entries by absolute path. |

`Walker` implements `Iterator<Item = Result<Entry, Error>>`. It yields files only. Directory symlinks and file symlinks are skipped; a symlink supplied as the root is rejected.

### `Error`

`Error` is the non-exhaustive error enum returned by filesystem, archive, and decompression operations. It implements `std::error::Error` and `Display`.

| Variant | Meaning |
| --- | --- |
| `Io(std::io::Error)` | A filesystem operation failed. |
| `MissingContent` | Contents were not embedded (`read = false`). |
| `NotADirectory(PathBuf)` | The runtime root is not a directory. |
| `NotOnDisk` | Disk metadata was requested for a non-disk entry. |
| `Decompress(&'static str)` | A ULZ1 payload is truncated, oversized, or inconsistent. |
| `InvalidPath(String)` | A path is a symlink, escapes its root, or is not a regular file. |
| `Limit(&'static str)` | A file-count, depth, or size limit was reached. |
| `Archive(String)` | An archive is unsupported, corrupt, or contains an invalid member. |

Because `Error` is marked `#[non_exhaustive]`, downstream exhaustive matches must include a wildcard arm.

### Compression API

The ULZ1 compression helpers are also re-exported from the crate root.

| Item | Signature | Description |
| --- | --- | --- |
| `Level` | `enum Level { Fast, Balanced, Max }` | Selects the encoder's match-search effort. |
| `MAX_UNCOMPRESSED` | `usize` constant | Maximum payload size accepted by the compression and decompression functions: 64 MiB. |
| `compress` | `fn compress(input: &[u8]) -> Result<Vec<u8>, &'static str>` | Compress using `Level::Balanced`. |
| `compress_with` | `fn compress_with(input: &[u8], level: Level) -> Result<Vec<u8>, &'static str>` | Compress using the selected level. |
| `compress_many` | `fn compress_many(inputs: &[&[u8]], level: Level) -> Result<Vec<Vec<u8>>, &'static str>` | Compress multiple buffers, preserving input order; multiple buffers are processed across available worker threads. |
| `decompress` | `fn decompress(input: &[u8]) -> Result<Vec<u8>, &'static str>` | Decode a ULZ1 payload, rejecting malformed data and payloads over the limit. |

`compress = fast`, `compress = true` (balanced), and `compress = max` are the macro-level equivalents. `Entry::content()` handles decompression automatically for entries created by the macro or by `Entry::compressed`.

### Archive API

These archive helpers are re-exported from the crate root:

| Item | Signature | Description |
| --- | --- | --- |
| `Node` | `struct Node { pub path: String, pub bytes: Vec<u8> }` | One regular file expanded from an archive. `path` is relative to the archive and uses `/`. |
| `is_archive_name` | `fn is_archive_name(name: &str) -> bool` | Check for a supported archive filename extension. |
| `is_archive_bytes` | `fn is_archive_bytes(bytes: &[u8]) -> bool` | Check the payload for a supported archive signature. |
| `expand` | `fn expand(bytes: &[u8], hint: &str, archive_entry: bool) -> Result<Vec<Node>, String>` | Expand archive bytes into regular-file nodes, recursively expanding nested archives up to the nesting limit. |

`hint` supplies the filename/extension used when identifying formats that cannot be distinguished from their content alone. `archive_entry` controls whether nested archive files are returned as nodes in addition to their members. The top-level archive is not included by `expand`.

## Limits and safety behavior

| Operation | Limit / behavior |
| --- | --- |
| `bind_dir!` traversal | 100,000 files; 64 directory levels; symlinks skipped. |
| `bind_dir!` embedded contents | 64 MiB per file; 256 MiB total. |
| Runtime `Statics` traversal | 100,000 files; 64 directory levels; symlinks skipped; root must be a real directory. |
| `Entry::content()` disk read | 512 MiB per file; symlinks and non-regular files are refused. |
| ULZ1 compression/decompression | 64 MiB uncompressed payload. |
| Archive expansion | 64 MiB archive/member; 256 MiB total expanded bytes; 100,000 members; eight nested levels. |

On Linux, disk reads open files with `O_NOFOLLOW` in addition to checking file metadata. Other platforms perform the metadata/symlink checks but do not use that Linux-specific open flag.

## Development

Clone the repository and run commands from its root. The checked-in `makefile` defines:

```sh
make test
make example
```

These invoke:

```sh
cargo test --workspace --all-targets
cargo run --example basic
```

The example demonstrates embedded assets, a runtime walk, static and owned buffers, and archive expansion. Integration tests are in `tests/embed.rs`; unit tests are alongside the implementation in `src/` and `ufs-macros/src/`.

## License

The package metadata declares `MIT OR Apache-2.0`. The supplied source tree does not contain separate `LICENSE-MIT` or `LICENSE-APACHE` text files.
