# ufs

[![Crates.io](https://img.shields.io/crates/v/ufs?style=flat-square)](https://crates.io/crates/ufs)
[![Documentation](https://img.shields.io/docsrs/ufs?style=flat-square)](https://docs.rs/ufs)
[![License](https://img.shields.io/crates/l/ufs?style=flat-square)](https://crates.io/crates/ufs)

**UFS (Universal File System)** is a small Rust library for working with files from two sources through a common `Entry` type:

- directories whose files are embedded into your application at compile time;
- directories scanned from the local filesystem at runtime.

It also provides optional support for listing and reading files inside common archive formats.

## Features

- **Embed a directory** with the `bind_dir!` procedural macro.
- **Walk a directory at runtime** recursively with `Statics`.
- **Work with a common entry type** containing a path, filename, size, depth, and content access.
- **Register multiple sources** in a `UFS` collection and retrieve them by a key.
- **Read archive entries** when the corresponding archive feature is enabled.

UFS is a read-oriented file discovery and access utility. It does not extract archives to disk or provide filesystem write operations.

## Subcrates

The repository is split into these supporting crates:

| Crate | Version |
| --- | --- |
| `ufs-base` | `0.1.0` |
| `ufs-macros` | `0.1.4` |

Most applications should depend on the top-level `ufs` crate. It re-exports the core API and the `bind_dir!` macro.

## Installation

Add UFS to your `Cargo.toml`:

```toml
[dependencies]
ufs = "0.1.5"
```

The default feature set is empty. To enable archive support, select the formats your application needs.

### Archive features

| Feature | Formats enabled |
| --- | --- |
| `7z` | `.7z` |
| `zip` | `.zip` |
| `tar` | `.tar` |
| `tgz` | `.tgz`, `.tar.gz` |
| `txz` | `.txz`, `.tar.xz` |
| `archive-all` | All of the formats above |

For example, enable ZIP and gzip-compressed TAR support:

```toml
[dependencies]
ufs = { version = "0.1.5", features = ["zip", "tgz"] }
```

Or enable every supported archive backend:

```toml
[dependencies]
ufs = { version = "0.1.5", features = ["archive-all"] }
```

Archive APIs are only compiled when at least one archive feature is enabled. The `is_archive()` extension method recognises known file extensions; make sure the matching feature is enabled before opening a file of that format.

## Quick start

The following example registers the same sample directory in two ways: as compile-time embedded files and as a directory walked from disk.

```rust
use ufs::{bind_dir, Bind, Statics, UFS};

#[derive(PartialEq, Eq)]
enum Source {
    Embedded,
    OnDisk,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut files = UFS::new();

    // The files are included in the executable when this crate is compiled.
    files.add(Source::Embedded, Bind::Embeds(bind_dir!("examples/files")))?;

    // This directory is read from the filesystem when the program runs.
    let disk_path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/files");
    files.add(Source::OnDisk, Bind::Statics(Statics::new(disk_path)))?;

    let embedded = files
        .get(Source::Embedded)?
        .embeds()
        .expect("the Embedded key contains embedded files");

    for entry in embedded.iter() {
        println!("embedded: {} ({} bytes)", entry.path.display(), entry.size);
    }

    let on_disk = files
        .get(Source::OnDisk)?
        .statics()
        .expect("the OnDisk key contains a filesystem directory");

    for entry in on_disk {
        println!("on disk:  {} ({} bytes)", entry.path.display(), entry.size);
    }

    Ok(())
}
```

Run the repository's quick-start example with:

```sh
cargo run --example basic
```

## Embedding a directory

Use `bind_dir!` to discover files while compiling. By default, UFS embeds their contents using `include_bytes!`, so the files are available even when the original directory is not present on the target machine.

```rust
use ufs::bind_dir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let assets = bind_dir!("examples/files");

    // Use .iter() when you want a fresh, repeatable iterator.
    for entry in assets.iter() {
        let bytes = entry.content()?;

        println!(
            "{} — {} bytes (depth {})",
            entry.path.display(),
            bytes.len(),
            entry.depth,
        );
    }

    Ok(())
}
```

The macro path is relative to `CARGO_MANIFEST_DIR` by default. The macro also accepts these options:

| Option | Default | Meaning |
| --- | --- | --- |
| `root` | `false` | Include the directory being embedded as the first component of each stored entry path. |
| `absolute` | `false` | Treat the supplied path as an absolute path instead of joining it to `CARGO_MANIFEST_DIR`. |
| `read` | `true` | Controls whether file bytes are embedded. The default is recommended when you need `Entry::content()`. |

For example, to include the directory name in the stored paths:

```rust
let assets = ufs::bind_dir!("examples/files", root = true);
```

With the default `root = false`, a file at `examples/files/nested/demo.txt` is stored as `nested/demo.txt`. With `root = true`, it is stored as `files/nested/demo.txt`.

## Walking a local directory

Use `Statics` when the files should be discovered at runtime, rather than included in the executable. `walk()` recursively traverses subdirectories and returns a `Result` containing an iterator of `Entry` values.

```rust
use ufs::Statics;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/files");

    for entry in Statics::new(root).walk()? {
        let contents = entry.content()?;
        println!(
            "{} — {} bytes",
            entry.path.display(),
            contents.len(),
        );
    }

    Ok(())
}
```

Unlike embedded entries, local entries are read from disk when `content()` is called. The directory must therefore exist and be readable when your application runs.

## Working with entries

Both embedded and local directory traversal produce `Entry` values. The fields and methods most commonly used are:

| Member | Description |
| --- | --- |
| `path` | `PathBuf` containing the entry path. |
| `filename` | File name as a `String`. |
| `size` | File size in bytes, collected during discovery. |
| `depth` | Path depth recorded by the walker or embedding macro. |
| `content()` | Returns the file bytes as `Result<&'static [u8], ufs::Error>`. |
| `is_local()` | Indicates whether the entry refers to an existing local file rather than embedded content. |
| `is_absolute()` | Reports whether `path` is absolute. |
| `metadata()` | Reads filesystem metadata for `path`; this requires that the path exists on the running machine. |

For embedded files, use the stored `size` and `content()` values when you do not need filesystem metadata for the original source path.

## Registering sources with `UFS`

`UFS<T>` is a small key-to-source registry. The key type must implement `PartialEq + Send + Sync`. Each key maps to either `Bind::Embeds` or `Bind::Statics`.

```rust
use ufs::{bind_dir, Bind, Statics, UFS};

#[derive(PartialEq, Eq)]
enum AssetGroup {
    BuiltIn,
    UserFiles,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut fs = UFS::new();

    fs.add(AssetGroup::BuiltIn, Bind::Embeds(bind_dir!("examples/files")))?;
    fs.add(
        AssetGroup::UserFiles,
        Bind::Statics(Statics::new("./user-files")),
    )?;

    let built_in = fs
        .get(AssetGroup::BuiltIn)?
        .embeds()
        .expect("BuiltIn is an embedded source");

    for entry in built_in.iter() {
        println!("built-in: {}", entry.filename);
    }

    let user_files = fs
        .get(AssetGroup::UserFiles)?
        .statics()
        .expect("UserFiles is a filesystem source");

    for entry in user_files {
        println!("user file: {}", entry.path.display());
    }

    Ok(())
}
```

`UFS::add` returns `Error::KeyExists` if a key is already registered. `UFS::get` returns `Error::KeyNotFound` for an unknown key. The `Bind::statics()` convenience method returns an `Option<Walker>`; use `Statics::walk()` directly when you need to handle filesystem errors.

## Reading archive contents

Enable an archive feature in `Cargo.toml` before importing `AchiveExt` (the trait name is spelled this way in the current API). Then call `is_archive()` and `archive()` on an `Entry`.

The following example searches a directory supplied on the command line, lists the files in each recognised archive, and reads each member's contents into memory:

```rust
use std::path::PathBuf;
use ufs::{AchiveExt, Statics};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./archives"));

    for file in Statics::new(directory).walk()? {
        if !file.is_archive() {
            continue;
        }

        println!("Archive: {}", file.path.display());
        let mut archive = file.archive()?;

        archive.entries(|mut member| match member.content() {
            Ok(bytes) => println!("  {} ({} bytes)", member.path(), bytes.len()),
            Err(error) => eprintln!("  could not read {}: {error}", member.path()),
        })?;
    }

    Ok(())
}
```

Run it with all archive formats enabled and pass a directory containing archives:

```sh
cargo run --example archives --features archive-all -- ./path/to/archives
```

`ArchiveEntry` exposes `path()`, `size()`, `metadata()`, `content()`, and `data()`. `content()` returns an owned `Vec<u8>` for the archive member. The example reads members into memory; it does not write them to disk.

## Examples in this repository

The `examples/` directory contains small programs built around the sample files in `examples/files/`:

| Example | What it demonstrates | Command |
| --- | --- | --- |
| `basic` | Registers embedded and local sources in `UFS`. | `cargo run --example basic` |
| `embed_dir` | Embeds a directory and reads file contents. | `cargo run --example embed_dir` |
| `static_dir` | Recursively walks a directory on disk. | `cargo run --example static_dir` |
| `registry` | Uses enum keys to retrieve different source types. | `cargo run --example registry` |
| `archives` | Lists archive members (requires archive features). | `cargo run --example archives --features archive-all -- ./archives` |

## Development

Format the workspace and run the package tests with:

```sh
cargo fmt --all
cargo test --all-features
```

To run the archive example against your own files:

```sh
cargo run --example archives --features archive-all -- /path/to/directory
```

## License

Licensed under either of the following, at your option:

- Apache License, Version 2.0
- MIT License
