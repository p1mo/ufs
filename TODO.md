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

- [ ] Investigate whether all existing archive implementations and dependencies could be replaced with the latest `hivepack` version (`0.14.0`).

Potential benefits:
- Use hivepack as a unified backend for all supported archive formats.
- Replace individual archive crates wherever hivepack provides suitable functionality.
- Map project-level feature flags to the corresponding hivepack features.
- Reduce dependency duplication, maintenance overhead, and unnecessary compilation.
- Keep unsupported or unsuitable formats independently implemented where necessary.

## Nested Archive Reading & Depth Control
- [ ] Add a configurable `Option<usize>` setting to control nested archive reading.
- [ ] Always read the root archive when its format is enabled.
- [ ] Support the following depth behavior:

| Setting | Behavior |
|---|---|
| `None` | Disable nested archive reading. |
| `Some(0)` | Unlimited nested archive reading. |
| `Some(1..)` | Read nested archives up to the specified depth. |

## Macro Improvements
- [ ] Make all macro arguments optional, except for `path`, which must remain required.

## Remote Filesystem Reading
- [ ] Add optional remote filesystem support for reading arbitrary files, not just archives.
- [ ] Evaluate **Apache OpenDAL (`opendal`)** as the unified remote storage backend.
- [ ] Support remote files through the existing required `path` argument.
- [ ] Provide a unified interface for local and remote file reading.
- [ ] Allow each remote storage provider to be enabled independently through Cargo feature flags.
- [ ] Keep remote filesystem dependencies optional.
- [ ] Support arbitrary file types, including archives, images, documents, videos, audio, and binary files.

### Supported Remote Backends

| Feature Flag | OpenDAL Feature | Storage / Protocol |
|---|---|---|
| `remote-http` | `services-http` | HTTP / HTTPS |
| `remote-sftp` | `services-sftp` | SFTP |
| `remote-ftp` | `services-ftp` | FTP |
| `remote-webdav` | `services-webdav` | WebDAV |
| `remote-smb` | `services-smb` | SMB / NAS |
| `remote-s3` | `services-s3` | AWS S3, MinIO, Cloudflare R2 |
| `remote-gcs` | `services-gcs` | Google Cloud Storage |
| `remote-azure` | `services-azblob` | Azure Blob Storage |

### General File Reading
- [ ] Read arbitrary files directly from remote storage without requiring manual downloads.
- [ ] Support reading files as byte streams.
- [ ] Support ranged reads and seeking wherever the backend allows them.
- [ ] Support listing remote directories and retrieving file metadata where available.
- [ ] Preserve the existing local filesystem reading functionality.
- [ ] Allow existing and future file format readers to consume remote data.
- [ ] Use temporary caching as a fallback when a format requires seeking or the backend lacks the required capabilities.

### Archive Integration
- [ ] Integrate remote reading with the existing archive detection and extraction pipeline.
- [ ] Reuse existing archive feature flags and nested archive depth configuration.
- [ ] Allow nested archives to be read directly from remote sources where supported.

### Authentication & Reliability
- [ ] Support provider-specific authentication and configuration.
- [ ] Support custom endpoints for S3-compatible storage.
- [ ] Handle authentication failures, network errors, timeouts, and interrupted reads.
- [ ] Never expose credentials or access tokens in logs.
- [ ] Respect individual backend capabilities instead of assuming every provider supports identical operations.
- [ ] Add mock-based and integration tests for supported providers.
