use ufs::{ bind_dir, UnifiedFS };



fn main() -> anyhow::Result<()> {

    let fs1 = bind_dir!("examples/files");

    for item in fs1.iter() {

        println!("embed > {:?}", item);

        println!("embed > {:?}", item.path.exists());
    }

    let fs2 = UnifiedFS::new();

    for item in fs2.walk(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/files")) {
        println!("local > {:?}", item);
    }

    loop {
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }

} 