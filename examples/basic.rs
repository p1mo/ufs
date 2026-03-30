use ufs::{ bind_dir, UnifiedFS };






fn embed() -> anyhow::Result<()> {

    let fs1 = bind_dir!("examples/files", read = false);

    for item in fs1.iter() {

        println!("embed > {:?}", item);

        println!("embed > {:?}", item.path.exists());
    }

    for item in fs1.walk(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/files")) {

        println!("local > {:?}", item);

        println!("local > {:?}", item.path.exists());
    }

    Ok(())

}

fn local() -> anyhow::Result<()> {

    let fs2 = UnifiedFS::new();

    for item in fs2.walk(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/files")) {
        println!("local > {:?}", item);
    }

    Ok(())

}



fn main() -> anyhow::Result<()> {

    embed()?;
    //local()?;

    loop {
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
} 