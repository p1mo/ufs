# TODO

---

## Feature Flags
- [ ] Put Rayon behind an optional feature flag (`parallel`). Without it, `par_iter()` / `IntoParallelIterator` are not exposed and internals (`read_all`, `expand_archives`, `Statics::files`) fall back to sequential code.
- [ ] Put archive reading behind an optional feature flag.
- [ ] Put compression behind an optional feature flag.
- [ ] Allow individual archive formats to be enabled independently.
- [ ] Allow individual compression algorithms to be enabled independently.
- [ ] Avoid unnecessary dependencies by only compiling enabled features.
- [ ] Forward every `ufs` feature to `ufs-macros`. The macro crate currently always compiles `flate2`, `lzma-rs`, `sevenz-rust2`, `tar` and `zip`, even when `archives` is never used.
- [ ] Trim dependency features: `sevenz-rust2` is built with `compress` although the crate only reads 7z; audit the default features of `zip`, `flate2` and `tar` the same way.
- [ ] Decide the `default` feature set. Keep today's behavior as the default for the 0.1.x line, and slim it down in the next minor release.
- [ ] Emit a clear `compile_error!` when a macro option needs a feature that is not enabled (for example `archives = true` without any `archive-*` feature).
- [ ] Add `async` / `tokio` features (see [Async Support](#async-support-sync--async)).
- [ ] Document the full feature matrix in the README and add `[package.metadata.docs.rs] all-features = true` with `doc(cfg(...))` badges.

## Compression & Performance
- [ ] Enhance ULZ1 compression with additional high-performance compression and decompression crates.
- [ ] Evaluate major, well-maintained Rust compression libraries.
- [ ] Prioritize compression speed, decompression speed, efficiency, and low resource overhead.
- [ ] Keep ULZ1 as the zero-dependency default. Other algorithms keep their own magic bytes so `Entry::content()` can detect the algorithm without a per-entry tag.
- [ ] Add an `algorithm` option to `bind_dir!` (see [Macro Improvements](#macro-improvements)). The generated code should only reference constructors that exist when the matching feature is on, so a missing feature becomes a normal compile error.
- [ ] Add `decompress_with_limit` so runtime decompression uses the entry's declared size as its limit instead of the fixed `MAX_UNCOMPRESSED` (required once macro limits are configurable).
- [ ] Look at embedding size: de-duplicate identical payloads (by content hash) and skip compression for already-compressed formats (png, jpg, zip, mp4, ...).
- [ ] Add `criterion` benchmarks for every algorithm and level (ratio, compress speed, decompress speed).

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
- [ ] Investigate additional archive formats supported by Rust crates (for example `tar.zst`, `tar.bz2`).
- [ ] Avoid pulling in archive formats that are not explicitly enabled.
- [ ] Replace the duplicated extension lists (`is_archive_name`) and magic-byte checks (`detect`) with one format table that is gated per feature.
- [ ] Replace the `String` errors in `archive.rs` with a structured error (`Escape`, `TooLarge`, `TooMany`, `Nesting`, `Unsupported`, `Corrupt`) so callers can match on them and the macro can format them.
- [ ] Add a compression-ratio limit (`max_ratio`) as an additional zip-bomb guard next to the size limits.

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

- [ ] Replace the hard-coded `MAX_NEST = 8` in `archive.rs` with this setting. The default stays `Some(8)` so current behavior does not change.
- [ ] `Some(0)` (unlimited) is still bounded by the file-count, member-size and total-size limits.
- [ ] Decide what happens at the depth cap: keep the nested archive as a plain file (current behavior) or return an error.
- [ ] Expose the setting in the macro (`archive_depth`), on `Statics`, and on `Entry::archive` / `UFS::expand_archives`.
- [ ] Optional: an enum (`Off`, `Depth(n)`, `Unlimited`) may be clearer than `Option<usize>` where `0` means unlimited. Keep `Option<usize>` if the current API is preferred.

## Macro Improvements
- [x] Make all macro arguments optional, except for `path`, which must remain required. *(Already the case in `parse_args`; keep it covered by tests.)*
- [ ] Replace the hand-written option parser (`seen: [bool; 6]` with magic slot numbers `0..5`, and a separate special case for `compress`) with one table-driven option registry (name, value kind, default, docs). The same table drives parsing, duplicate detection, "unknown option, did you mean ...?" errors and the option table in the `bind_dir!` docs.
- [ ] Parse real `proc_macro` tokens (string, integer and ident literals) instead of `input.to_string()` followed by a custom unescaper. This keeps spans, so errors can point at the exact option instead of the whole macro call. Make `take_string` / `take_raw_string` unnecessary.
- [ ] Emit byte data with `proc_macro::Literal::byte_string` (stable since Rust 1.79, MSRV is 1.85) instead of building a huge `b"..."` string with `format!` and re-parsing it. This should cut compile time and memory for large or compressed payloads.
- [ ] Build error messages from the active limits (they currently contain literal "64 MiB", "256 MiB", "100000 files", "8 levels", "64 levels").
- [ ] Rebuild tracking: a newly added file in the embedded directory does not trigger a rebuild, because only existing files are tracked with `include_bytes!`. Use `proc_macro::tracked_path` where available, or document / provide a `build.rs` helper (`cargo:rerun-if-changed=<dir>`).
- [ ] Respect `CARGO_TARGET_DIR` and a configurable target path in the "refuse to embed target dir" check instead of assuming `<manifest>/target`.
- [ ] Add optional filters: `include = "**/*.png"`, `exclude = "**/*.tmp"`, `hidden = false` (skip dot-files), and a `follow_symlinks = false` switch (default stays "skip symlinks").
- [ ] Add `algorithm = ulz1 | lz4 | zstd | ...` once more compression backends exist; `compress = fast | balanced | max` keeps working and means "ULZ1 at that level" by default.
- [ ] Sort generated entries by path (already done for directory files, but archive members are appended afterwards) and let `UFS::get` use binary search or a lazily built index instead of a linear scan.
- [ ] Add `trybuild` compile-fail tests so every macro error message and span is tested.

## Configurable Macro Limits
- [ ] Make all hardcoded limits inside macros configurable and overridable by the user.
- [ ] Allow users to adjust internal limits through optional macro arguments or configuration options.
- [ ] Keep sensible default values while allowing users to customize them without modifying the library source code.
- [ ] Introduce one shared `Limits` struct (with `Default`) instead of separate constants in `ufs-macros/src/lib.rs`, `archive.rs`, `compress.rs` and `statics.rs`.
- [ ] Reject nonsense values with a clear compile error (zero, overflow, above the absolute ceiling). The absolute ceilings stay fixed: ULZ1 stores its length as `u32`, and `usize` arithmetic must not overflow.
- [ ] Accept plain integers and size units, for example `max_total = 512 * 1024 * 1024` or `max_total = "512MiB"`.

Current hardcoded limits and proposed overrides:

| Current limit | Where | Default | Proposed macro option |
|---|---|---|---|
| `MAX_FILES` | macro, `archive.rs`, `statics.rs` | 100,000 | `max_files` |
| `MAX_DEPTH` (directory depth) | macro, `statics.rs` | 64 | `max_depth` |
| `MAX_TOTAL` | macro, `archive.rs` | 256 MiB | `max_total` |
| `compress::MAX_UNCOMPRESSED` (per embedded file) | macro, `compress.rs` | 64 MiB | `max_file_size` |
| `archive::MAX_MEMBER` | `archive.rs` | 64 MiB | `max_member_size` |
| `archive::MAX_NEST` | `archive.rs` | 8 | `archive_depth` (see nested archive section) |
| `Entry::MAX_DISK_READ` (runtime only) | `entry.rs` | 512 MiB | `Statics::max_read(..)` builder method |

Also configurable (defaults stay as they are):

| Current default | Proposed option |
|---|---|
| `read = true` | `read` (already exists) |
| `archive_entry = true` | `archive_entry` (already exists) |
| Hash-chain depth of `fast` / `balanced` / `max` = 1 / 32 / 256 | stays tied to `compress` levels; the ULZ1 format constants (`MIN_MATCH`, `MAX_MATCH`, `MAX_DISTANCE`) are part of the file format and stay fixed |

- [ ] Runtime counterpart: `Statics::new(path).limits(Limits { .. })`, `Entry::archive_with(limits)`, so runtime and compile-time limits behave the same.
- [ ] Optional: allow workspace-wide defaults through an environment variable (`UFS_MAX_TOTAL=...`) or `[package.metadata.ufs]`.
- [ ] Add tests for every limit: at the limit passes, one over fails, and the error message names the configured value.

## Async Support (Sync + Async)
The crate stays synchronous by default. Async is added on top through feature flags, without a second implementation of the security checks.

- [ ] Keep the existing sync API unchanged and enabled without any async feature. Async is purely additive.
- [ ] Split a sans-IO core (pure functions on bytes: decompress, archive detect / expand, path sanitize, limits) from thin I/O adapters. The sync adapter uses `std::fs`; the async adapter uses `tokio::fs`. Both call the same core, so the symlink, path-escape and size checks cannot drift apart.
- [ ] Do not make `bind_dir!` async. It runs at compile time and stays sync.

### Feature flags

| Feature Flag | Purpose |
|---|---|
| `async` | Runtime-agnostic API surface: stream type (`futures-core`), `futures-io` reader traits. No executor dependency. |
| `tokio` | Implementation on `tokio::fs` + `tokio::task::spawn_blocking`. Implies `async`. |
| `smol` / `async-std` | Optional later; the core design must not depend on tokio types in the public API. |

### API sketch
- [ ] `Entry::content_async()` returns `Cow<[u8]>` like `content()`. Embedded (`Bytes`, `Owned`) and `Omitted` entries resolve immediately without spawning anything; only `Disk`, large `Compressed` payloads and archives do real async work.
- [ ] `Entry::archive_async()`, `UFS::read_all_async(concurrency)`, `UFS::expand_archives_async(keep)`.
- [ ] `Statics::load_async()` and `Statics::walk_stream()` returning `impl Stream<Item = Result<Entry, Error>>`.
- [ ] Naming: `_async` suffix on the existing types (simple, discoverable) vs a mirrored `ufs::asynch` module. Decide before the first release.
- [ ] Streaming reads for large files: `Entry::reader()` returns `impl Read` (sync) and `impl AsyncRead` (async), so a file no longer has to be loaded completely into memory (today up to 512 MiB).
- [ ] Phase 1 for the async walk: run the existing sync `Walker` inside `spawn_blocking` and send entries through a bounded channel. Phase 2: native `tokio::fs::read_dir` with bounded concurrency.

### Rules to follow
- [ ] CPU-heavy work (ULZ1 / zstd decompression, archive expansion, compression, anything using Rayon) must run through `spawn_blocking` or a oneshot from `rayon::spawn`. Never call `par_iter()` directly on an async worker thread. Use a size threshold (for example 64 KiB) below which decompression runs inline.
- [ ] Public futures and streams are `Send + 'static`-friendly. Avoid bare `async fn` in public traits; return `impl Future + Send` or use `trait-variant`. Add compile-time `assert_send_sync` tests for `Entry`, `UFS` and every returned future.
- [ ] `tokio::fs::OpenOptions` supports `custom_flags` on Unix, so the async path can reuse the same no-follow open. Share one validation function (symlink check, regular file, size limit) between sync and async.
- [ ] Document cancellation safety for each async method. Timeouts are the caller's responsibility (`tokio::time::timeout`), except for remote backends, which get configurable timeouts.
- [ ] Remote reading (OpenDAL) is async-native. Build it on the `tokio` feature and offer the sync API through a blocking adapter. Detect an already running runtime (`Handle::try_current`) so the blocking adapter never calls `block_on` inside async code.

### Tests & benchmarks
- [ ] Parity tests: run the same fixtures (`examples/files`, `examples/archives`) through the sync and async APIs and compare results (`#[tokio::test]`).
- [ ] Doctests for both flavors, gated with `#[cfg(feature = "...")]`.
- [ ] `criterion` benchmark: sync vs async overhead and the best `spawn_blocking` threshold.

## Remote Filesystem Reading
- [ ] Add optional remote filesystem support for reading arbitrary files, not just archives.
- [ ] Evaluate **Apache OpenDAL (`opendal = "0.59.5"`)** as the unified remote storage backend.
- [ ] Support remote files through the existing required `path` argument.
- [ ] Provide a unified interface for local and remote file reading.
- [ ] Allow each remote storage provider to be enabled independently through Cargo feature flags.
- [ ] Keep remote filesystem dependencies optional.
- [ ] Support arbitrary file types, including archives, images, documents, videos, audio, and binary files.
- [ ] Build the remote layer on the async support above (`remote-*` features imply `tokio`).

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
- [ ] Apply the same `Limits` (max file size, max total, max files) to remote reads.

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

## Crate Layout & Publishing
- [ ] `ufs-macros` includes `../../src/archive.rs` and `../../src/compress.rs` with `#[path = ...]`. Files outside the crate directory are not part of the published package, so the published `ufs-macros` may not build. **Verify** with `cargo package --list -p ufs-macros` and `cargo publish --dry-run`.
- [ ] Fix by moving the shared, sans-IO code (compress, archive, `Limits`, format table) into an internal crate (`ufs-base`) that both `ufs` and `ufs-macros` depend on. This also removes the `#[allow(dead_code)]` workarounds and makes feature forwarding straightforward.
- [ ] Add `cargo package` / `cargo publish --dry-run` to CI for both crates.

## Hardening & Correctness (found while reviewing the code)
- [ ] `open_nofollow` hard-codes `0x20000` as `O_NOFOLLOW` for `target_os = "linux"`. The value differs between architectures (for example arm / aarch64 use another value), so the protection could silently not apply there. **Verify on aarch64**, then use `libc::O_NOFOLLOW` (a constant, no `unsafe` needed) or `rustix` / `cap-std`.
- [ ] No no-follow open exists for macOS / BSD (the code is Linux-only) or Windows. Use `cfg(unix)` with `libc::O_NOFOLLOW`, and a reparse-point check on Windows.
- [ ] `read_disk` checks `symlink_metadata` and then opens the file (time-of-check / time-of-use gap). After opening, check `file.metadata()` on the handle (regular file, size limit) instead.
- [ ] `UFS::get` scans all entries (O(n)). Use binary search on sorted entries or a lazily built index.
- [ ] `Entry` has public fields (`size`, `depth`, `path`, `filename`). Switch to accessor methods and `#[non_exhaustive]` so the layout can change without a breaking release.
- [ ] Reduce overlapping constructors (`UFS::embedded` / `UFS::entry`, `statics` / `r#static`) and mark duplicates as deprecated before 0.2.
- [ ] Add a streaming reader for large disk files (see Async Support) and an optional `mmap` feature.

## Testing & CI
- [ ] `trybuild` compile-fail tests for the macro (messages and spans).
- [ ] `proptest` round-trip tests for ULZ1 and every added compression algorithm.
- [ ] `cargo-fuzz` targets for `decompress`, `detect` and `expand` (untrusted input paths).
- [ ] Run `miri` on the compression code.
- [ ] CI matrix: Linux, macOS, Windows, Linux aarch64; MSRV job (Rust 1.85); `cargo hack --feature-powerset check` so no feature combination breaks.
- [ ] CI commands: `cargo fmt --check`, `cargo clippy --all-targets --all-features`, `cargo test --workspace --all-features`, `cargo test --doc`, plus `cargo deny` / `cargo audit`.