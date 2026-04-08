use ufs::{ bind_dir, UnifiedFS, AchiveExt };



fn main() -> anyhow::Result<()> {

    let fs1 = bind_dir!("examples/files");

    for item in fs1.iter() {

        println!("embed > {:?}", item.path);

        if item.is_archive() {

            item.archive()?.entries(|mut entry| {

                println!("embed archive > {:?} > {}", entry.path(), entry.content().unwrap().len());
            
            })?;

        }
    }

    let fs2 = UnifiedFS::new();

    for item in fs2.walk(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/files")) {

        println!("local > {:?}", item.path);
        
    }

    loop {
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }

} 