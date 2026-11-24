# ufs-macros

Procedural macro behind [`ufs::bind_dir`](https://docs.rs/ufs).

Only the path is required. Every option is optional:

| Option | Default |
| --- | --- |
| `read` | `true` |
| `root` | `false` (set it to keep the directory name on each stored path) |
| `absolute` | `false` |
| `compress` | `false`. Levels: `fast`/`min`, `balanced`/`true`/`default`, `max`/`best`/`high`. `content()` decompresses any of them. |
| `archives` | `false` |
| `archive_entry` | `true` |

`..` in the path is allowed. ULZ1 and the archive readers are `ufs/src/compress.rs` and `ufs/src/archive.rs`, included by this crate so `compress` and `archives = true` run at compile time.

Depend on `ufs`, not this crate, unless you are working on the macro itself. Archive decoding is `ufs/src/archive.rs`, compiled into this crate only so `archives = true` can run at compile time.
