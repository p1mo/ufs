use ufs::{ bind_dir, UnifiedFS };

fn main() -> anyhow::Result<()> {

    let fs1 = bind_dir!("examples/files");

    for item in fs1.iter() {
        println!("{:?}", item.path);
    }

    let fs2 = UnifiedFS::walk("examples/files")?;

    for item in fs2.iter() {
        println!("{:?}", item.path);
    }


    Ok(())
}