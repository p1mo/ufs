# ufs-base

[![Crates.io](https://img.shields.io/crates/v/ufs-base?style=flat-square)](https://crates.io/crates/ufs-base)

`ufs-base` contains the core types and runtime functionality used by the [`ufs`](https://crates.io/crates/ufs) crate.

It provides:

- `Entry`, the common representation of a discovered file;
- `Statics` and `Walker` for recursively walking a directory on disk;
- `Embeds` for working with entries generated at compile time;
- `Bind` and `UFS` for grouping and retrieving file sources;
- optional archive reading support through the `7z`, `zip`, `tar`, `tgz`, and `txz` features.

Most applications should depend on `ufs` rather than using this implementation crate directly. The `ufs` crate re-exports the public core API.
