<div align="center">

# ufs

**Small library for reading and embeding directorys.**

[![Crates.io](https://img.shields.io/crates/v/ufs?style=for-the-badge)](https://crates.io/crates/ufs)
[![Documentation](https://img.shields.io/docsrs/ufs?style=for-the-badge)](https://docs.rs/ufs)
[![License](https://img.shields.io/crates/l/ufs?style=for-the-badge)](https://crates.io/crates/ufs)

</div>

The old archive backends (`zip`, `tar`, `7z`, …) are gone but will be added again later on.

## What's coming in version 0.2

- Archives (will be added again)
- Parallelism

## Install

```toml
[dependencies]
ufs = "0.1"
```

## Embed a directory

```rust
use ufs::bind_dir;

let assets = bind_dir!("examples/files", compress = true);
let demo = assets.get("demo.txt").unwrap();
assert!(demo.content().unwrap().starts_with(b"Lorem ipsum"));
```

The macro path is relative to `CARGO_MANIFEST_DIR`.

| Option | Default | Meaning |
| --- | --- | --- |
| `read` | `true` | Embed bytes. `false` stores the path and size only. |
| `root` | `false` | Prefix stored paths with the directory name (`files/demo.txt` instead of `demo.txt`). |
| `absolute` | `false` | Treat the path as absolute. Required for anything outside the crate directory. |
| `compress` | `false` | Compress each file with ULZ1 when the result is smaller. Requires `read = true`. |

Symlinks are skipped, so a link cannot pull files in from outside the directory. `bind_dir!` also refuses the crate root, the filesystem root, and the Cargo target directory. Embeds are capped at 100_000 files, 64 MiB each, and 256 MiB total.

Changing a source file rebuilds the crate. Compressed payloads are cached in `target/ufs-cache` and still tracked back to the original file.

## Walk a directory

```rust
use ufs::Statics;

fn main() -> Result<(), ufs::Error> {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/files");
    for entry in Statics::new(root).files()? {
        println!("{} — {} bytes", entry.path.display(), entry.content()?.len());
    }
    Ok(())
}
```

The root must be a real directory, not a symlink. Links inside the tree are skipped. Depth is counted from that root, not from the absolute path. `content()` opens the file with `O_NOFOLLOW` on Linux and will not read anything larger than 512 MiB.

`walk()` is the lazy form and yields `Result<Entry, Error>` so one bad file does not panic or disappear.

## Register both

```rust
use ufs::{Bind, Statics, UFS, bind_dir};

fn main() -> Result<(), ufs::Error> {
    let mut files = UFS::new();
    files.add("built-in", Bind::Embeds(bind_dir!("examples/files")))?;
    files.add(
        "user",
        Bind::Statics(Statics::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/files"
        ))),
    )?;

    for entry in files.get(&"built-in")?.embeds().unwrap() {
        println!("embedded {}", entry.path.display());
    }
    Ok(())
}
```

## Run Example & Tests

```sh
cargo run --example basic
cargo test --workspace
```

## Layout

| Crate | Role |
| --- | --- |
| `ufs` | Runtime: `Entry`, `Embeds`, `Statics`, `UFS`, ULZ1 |
| `ufs-macros` | `bind_dir!`, no dependencies |

## ULZ1

`compress` / `decompress` are public. The blob is `b"ULZ1"`, a `u32` length, then literal runs and length/distance matches. `decompress` refuses a declared length above 64 MiB and returns an error on a truncated or inconsistent stream instead of panicking.

## License

MIT OR Apache-2.0
