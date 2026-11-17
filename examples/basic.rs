use ufs::{Bind, Statics, UFS, bind_dir};

#[derive(PartialEq, Eq, Debug)]
enum Source {
    Embedded,
    OnDisk,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut files = UFS::new();

    files.add(
        Source::Embedded,
        Bind::Embeds(bind_dir!("examples/files", compress = true, root = true)),
    )?;

    let disk_path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/files");
    files.add(Source::OnDisk, Bind::Statics(Statics::new(disk_path)))?;

    let embedded = files
        .get(&Source::Embedded)?
        .embeds()
        .expect("embedded source");
    for entry in embedded {
        let bytes = entry.content()?;
        println!(
            "embedded: {} ({} bytes, compressed={})",
            entry.path.display(),
            bytes.len(),
            entry.is_compressed()
        );
    }

    let on_disk = files
        .get(&Source::OnDisk)?
        .statics()
        .expect("filesystem source");
    for entry in on_disk.files()? {
        let bytes = entry.content()?;
        println!("on disk:  {} ({} bytes)", entry.path.display(), bytes.len());
    }

    Ok(())
}
