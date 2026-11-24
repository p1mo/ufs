# ufs

Embed a directory at compile time, or walk one at runtime.

## Install

```toml
[dependencies]
ufs = "0.2"
```

## Embed a directory

```rust
use ufs::bind_dir;

let assets = bind_dir!("examples/files", compress = true);
let demo = assets.get("demo.txt").unwrap();
assert!(demo.content().unwrap().starts_with(b"Lorem ipsum"));
```

The path is the only required argument. It is relative to `CARGO_MANIFEST_DIR`. `..` may leave the crate (`../other/assets`). `root = true` puts that directory's name on the stored path, so the entry is `assets/...` rather than a bare filename. `content()` always returns the original bytes: `fast`, `balanced`, and `max` use one decoder.

| Option | Default | Meaning |
| --- | --- | --- |
| `read` | `true` | Embed bytes. `false` stores the path and size only. |
| `root` | `false` | Prefix stored paths with the directory name (`files/demo.txt`). |
| `absolute` | `false` | Treat the path as absolute. |
| `compress` | `false` | Optional ULZ1 level: `fast` (or `min`), `balanced` (or `true` / `default`), `max` (or `best` / `high`). `false` / `off` / `none` disables it. Requires `read = true`. |
| `archives` | `false` | Expand zip, tar, tar.gz, tar.xz, and 7z into entries. Requires `read = true`. |
| `archive_entry` | `true` | With `archives`, also keep the archive file itself as an entry. |

```rust
let rooted = bind_dir!("examples/../examples/files", root = true, compress = fast);
assert!(rooted.get("files/demo.txt").unwrap().content().unwrap().starts_with(b"Lorem"));
```

Symlinks are skipped. The filesystem root, the crate root itself, and the Cargo target directory are refused. Embeds stop at 100,000 files, 64 MiB each, and 256 MiB total.

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

The root must be a real directory, not a symlink. Depth is counted from that root. Disk reads use `O_NOFOLLOW` on Linux and stop at 512 MiB.

## Archives

Supported types are zip, tar, `.tar.gz` / `.tgz`, `.tar.xz` / `.txz`, and 7z.

```rust
use ufs::bind_dir;

let packed = bind_dir!("examples/archives", archives = true, archive_entry = true);
let zip = packed.get("pack.zip").expect("archive file kept as an entry");
let hello = zip.archive().unwrap().get("pack.zip/hello.txt").unwrap();
assert_eq!(hello.content().unwrap().as_ref(), b"hello-zip");
assert!(packed.get("pack.zip/hello.txt").is_some());
```

`entry.archive()` lists the files inside one archive. `archives = true` also splices those files into the embed or the directory walk (`Statics::archives(true)`). `archive_entry = false` drops the archive file from that list; `true` keeps it beside its members. Nested archives expand up to 8 levels, and with `archive_entry` each nested archive is an entry too.

Member paths look like `pack.zip/hello.txt`. Paths containing `..` or a NUL are rejected. Tar symlinks are skipped. A member over 64 MiB, or an expand over 256 MiB, is an error.

## One type

`bind_dir!`, a disk walk, one embedded file, an owned buffer, and a static archive are all a `UFS`.

```rust
use ufs::{UFS, bind_dir};

fn main() -> Result<(), ufs::Error> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/files");
    let embedded: UFS = bind_dir!("examples/files");
    let user: UFS = UFS::statics(dir)?;
    let note: UFS = UFS::embedded("note.txt", b"hello");
    let scratch: UFS = UFS::bytes("scratch.txt", b"owned".to_vec());
    let pack: UFS = UFS::archive("pack.zip", include_bytes!("examples/archives/pack.zip"))?;

    for entry in &pack {
        let _ = entry.content()?;
    }
    let _ = user.read_all()?;
    let _ = (embedded, note, scratch);
    Ok(())
}
```

`UFS::statics` walks directories in parallel (`UFS::r#static` is the same function; `static` is a keyword). `read_all` and `expand_archives` also split work across cores. `UFS::archive` keeps the archive file and the files inside it.

## Layout

| Crate | Role |
| --- | --- |
| `ufs` | `Entry`, `UFS`, `Statics`, ULZ1, zip/tar/7z |
| `ufs-macros` | `bind_dir!` |

## License

MIT OR Apache-2.0
