<div style="display: flex; align-items: center; justify-content: center; flex-direction: column; width: 100%;">
    <h1>ufs</h1>
    <h3>embed dirs & walk local dirs</h3>
</div>

<div style="display: flex; align-items: center; justify-content: center; flex-direction: row; width: 100%; gap: 5px;">
    <div style="display: flex; flex-direction: column; align-items: center; padding: 5px 10px; background: rgba(0, 0, 0, 0.1); border-radius: 5px;">
        <b style="font-size: 12px; padding: 5px 10px;">ufs</b>
        <a href="https://crates.io/crates/ufs">
            <img src="https://img.shields.io/crates/v/ufs?style=flat-square">
        </a>
    </div>
    <div style="display: flex; flex-direction: column; align-items: center; padding: 5px 10px; background: rgba(0, 0, 0, 0.1); border-radius: 5px;">
        <b style="font-size: 12px; padding: 5px 10px;">ufs-macros</b>
        <a href="https://crates.io/crates/ufs-macros" style="margin: 0; padding: 0;">
            <img src="https://img.shields.io/crates/v/ufs-macros?style=flat-square">
        </a>
    </div>
</div>

<br>

**Extract Archives**
 + Endings Supported: `.7z` `.zip` `.tar` `.tar.gz` `.tgz` `.tar.xz` `.txz`
 + in terms of speed is the `tar` section the fastest.

<br>

### Examples

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