# ufs-macros

Procedural macro behind [`ufs::bind_dir`](https://docs.rs/ufs). It has no dependencies.

The macro parses its input by hand (no `syn` / `quote`). Compression uses ULZ1, copied from `ufs/src/compress.rs` so this crate can be packaged on its own.

Depend on `ufs`, not this crate, unless you are working on the macro itself.
