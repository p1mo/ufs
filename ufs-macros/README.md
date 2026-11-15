# ufs-macros

[![Crates.io](https://img.shields.io/crates/v/ufs-macros?style=flat-square)](https://crates.io/crates/ufs-macros)

`ufs-macros` provides the `bind_dir!` procedural macro used by the [`ufs`](https://crates.io/crates/ufs) crate. It scans a directory at compile time and generates an `Embeds` collection containing an `Entry` for each file.

```rust
use ufs::bind_dir;

let assets = bind_dir!("assets");
for entry in assets.iter() {
    println!("{} ({} bytes)", entry.path.display(), entry.size);
}
```

The macro accepts three optional settings:

- `read = true|false` controls whether file contents are embedded (default: `true`);
- `root = true|false` controls whether the embedded directory name is included in stored paths (default: `false`);
- `absolute = true|false` treats the supplied path as absolute instead of resolving it from `CARGO_MANIFEST_DIR` (default: `false`).

Most applications should use `bind_dir!` through the top-level `ufs` crate, which re-exports the macro.
