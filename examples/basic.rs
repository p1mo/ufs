use ufs::{UFS, bind_dir};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/files");

    let embedded: UFS = bind_dir!("examples/files", compress = true);
    let on_disk: UFS = UFS::statics(dir)?;
    let note: UFS = UFS::embedded("note.txt", b"hello from a struct");
    let scratch: UFS = UFS::bytes("scratch.txt", b"owned".to_vec());
    let pack: UFS = UFS::archive("pack.zip", include_bytes!("archives/pack.zip"))?;

    for (label, files) in [
        ("embedded", &embedded),
        ("on disk", &on_disk),
        ("note", &note),
        ("scratch", &scratch),
        ("archive", &pack),
    ] {
        for entry in files {
            let bytes = entry.content()?;
            println!(
                "{label}: {} ({} bytes, compressed={})",
                entry.path.display(),
                bytes.len(),
                entry.is_compressed()
            );
        }
    }

    Ok(())
}
