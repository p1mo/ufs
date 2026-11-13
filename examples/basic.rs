use ufs::{ Bind, Statics, UFS, bind_dir };

#[derive(PartialEq)]
pub enum MYFiles {
    Demo1,
    Demo2,
}

fn main() -> std::io::Result<()> {

    let mut fs = UFS::new();

    fs.add(MYFiles::Demo1, Bind::Embeds(bind_dir!("examples/files"))).unwrap();
    fs.add(MYFiles::Demo2, Bind::Statics(Statics::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/files")))).unwrap();

    let embeded_files = fs.get(MYFiles::Demo1).unwrap().embeds().unwrap();
 
    for item in embeded_files {

        println!("embed > {:?}", item.path);

    }

    let static_files = fs.get(MYFiles::Demo2).unwrap().statics().unwrap();

    for item in static_files {

        println!("local > {:?}", item.path);
        
    }

    loop {
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }

} 