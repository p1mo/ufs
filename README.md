<center>
    <h1>UFS</h1>
    <h4>Universal FS Tool</h4>
</center>

<center>
    <center style="line-height: 40px;">
        <b>ufs</b>
        <a href="https://crates.io/crates/ufs">
            <img src="https://img.shields.io/crates/v/ufs?style=flat-square">
        </a>
    </center>
    <center style="line-height: 40px;">
        <b>ufs-macros</b>
        <a href="https://crates.io/crates/ufs-macros" style="margin: 0; padding: 0;">
            <img src="https://img.shields.io/crates/v/ufs-macros?style=flat-square">
        </a>
    </center>
</center>

<hr>

**Extract Archives**
 + Endings Supported: `.7z` `.zip` `.tar` `.tar.gz` `.tgz` `.tar.xz` `.txz`
 + in terms of speed is the `tar` section the fastest.

**Features**
 + Embed `Dirs` and `Archives`
 + Walk Local `Dirs` and `Archives`
 + Parallel Walking via `rayon`

<br>

### Examples

<br>

**`Features`**

```toml
default = []
# Enable all
all = ["7z", "zip", "tar", "txz", "tgz"]
# Extensions
7z   = ["dep:sevenz-rust2"]
zip  = ["dep:zip"]
tar  = ["dep:tar"]
txz  = ["dep:tar", "dep:xz2"]
tgz  = ["dep:tar", "dep:flate2"]
```

<br>

**`Embed Dir`**

```rust
use ufs::bind_dir;

fn main() -> anyhow::Result<()> {

    let unifs = bind_dir!("examples/files");

    for item in unifs.iter() {
        println!("embed > {:?}", item);
    }

} 
```


**`Static Dir`**

```rust
use ufs::UnifiedFS;

fn main() -> anyhow::Result<()> {

    let unifs = UnifiedFS::new();

    for item in unifs.walk(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/files")) {
        println!("embed > {:?}", item);
    }

} 
```


**`Embed dir & walk Static`**

```rust
use ufs::{ bind_dir, UnifiedFS };

fn main() -> anyhow::Result<()> {

    // Embed Files
    let unifs = bind_dir!("examples/files");

    for item in unifs.iter() {
        println!("embed > {:?}", item);
    }

    // Walk in Local Folders
    for item in unifs.walk(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/files")) {
        println!("local > {:?}", item);
    }

} 
```


**`Extract Archives in Embed dir & walk Static`**

```rust
use ufs::{ bind_dir, UnifiedFS, AchiveExt };

fn main() -> anyhow::Result<()> {
    //for item in bind_dir!("examples/files").iter() {
    for item in unifs.walk(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/files")) {
        if item.is_archive() {
            item.archive()?.entries(|mut entry| {
                println!("{:?} > {:?}", item.path, entry.content().unwrap().len());
            })?;
        }
    }
} 
```