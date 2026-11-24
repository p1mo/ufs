# TODO — Feature Flags & Performance Improvements

## Feature Flags
- [ ] Put Rayon behind an optional feature flag.
- [ ] Put archive reading behind an optional feature flag.
- [ ] Put compression behind an optional feature flag.
- [ ] Allow individual archive formats to be enabled independently.
- [ ] Allow individual compression algorithms to be enabled independently.
- [ ] Avoid unnecessary dependencies by only compiling enabled features.

## Compression & Performance
- [ ] Enhance ULZ1 compression with additional high-performance compression and decompression crates.
- [ ] Evaluate major, well-maintained Rust compression libraries.
- [ ] Prioritize compression speed, decompression speed, efficiency, and low resource overhead.

### Example Compression Features

| Feature Flag | Rust Crate | Algorithm |
|---|---|---|
| `compression-lz4` | `lz4_flex` | LZ4 |
| `compression-zstd` | `zstd` | Zstandard |
| `compression-brotli` | `brotli` | Brotli |
| `compression-deflate` | `flate2` | Deflate / Gzip |
| `compression-ulz1` | Internal | ULZ1 |

*Examples only; final selection is open.*

## Archive Support

- [ ] Use `hivepack = "0.14.0"` as a potential unified archive backend.
- [ ] Support individual archive formats through separate project-level feature flags.
- [ ] Investigate additional archive formats supported by Rust crates.
- [ ] Avoid pulling in archive formats that are not explicitly enabled.

### Example Archive Features

| Feature Flag | Hivepack Feature | Archive Format |
|---|---|---|
| `archive-zip` | `zip` | ZIP |
| `archive-tar` | `tar` | TAR |
| `archive-7z` | `sevenz` | 7-Zip |
| `archive-rar` | `rar` | RAR (reading) |
| `archive-cpio` | `cpio` | CPIO |
| `archive-iso` | `iso` | ISO 9660 |
| `archive-udf` | `udf` | UDF |
| `archive-xar` | `xar` | XAR |

*Feature names should be verified against hivepack 0.14.0.*

### Possible Archive Backend Consolidation

**Investigate whether it makes sense to replace all existing archive implementations and dependencies with the latest hivepack version (`0.14.0`).**

The goal would be to:
- Use hivepack as the unified backend for all supported archive formats.
- Replace individual archive crates wherever hivepack provides suitable functionality.
- Map project-level feature flags directly to the corresponding hivepack features.
- Reduce dependency duplication, maintenance overhead, and unnecessary compilation.
- Keep unsupported or unsuitable formats independently implemented where necessary.

## Macro Improvements
- [ ] Make all macro arguments optional, except for `path`, which must remain required.