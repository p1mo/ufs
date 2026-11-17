use std::path::Path;

use ufs::{Bind, Statics, UFS, bind_dir};

#[test]
fn embedded_files_match_disk() {
    let embedded = bind_dir!("examples/files");
    let compressed = bind_dir!("examples/files", compress = true);
    let rooted = bind_dir!("examples/files", root = true);
    let names_only = bind_dir!("examples/files", read = false);

    assert_eq!(embedded.len(), 2);
    assert_eq!(embedded.iter().count(), 2);
    assert_eq!(embedded.iter().count(), 2);
    assert!(rooted.get("files/demo.txt").is_some());
    assert!(rooted.get("files/nested/big-demo.txt").is_some());

    let omitted = names_only.get("demo.txt").unwrap();
    assert!(omitted.size > 0);
    assert!(omitted.content().is_err());
    assert!(omitted.metadata().is_err());

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/files");
    for entry in &embedded {
        let raw = std::fs::read(root.join(&entry.path)).unwrap();
        assert_eq!(entry.content().unwrap().as_ref(), raw.as_slice());
        assert_eq!(entry.size, raw.len() as u64);
        assert!(!entry.is_local());
        assert!(!entry.is_absolute());
        assert_eq!(
            entry.depth,
            entry.path.components().count().saturating_sub(1)
        );

        let other = compressed.get(&entry.path).unwrap();
        assert_eq!(other.content().unwrap().as_ref(), raw.as_slice());
        if entry.path.ends_with("big-demo.txt") {
            assert!(
                other.is_compressed(),
                "repetitive fixture should shrink under ULZ1"
            );
        }
    }

    let disk = Statics::new(&root).files().unwrap();
    assert_eq!(disk.len(), embedded.len());
    for entry in disk {
        assert!(entry.is_local());
        assert!(entry.is_absolute());
        let raw = std::fs::read(&entry.path).unwrap();
        assert_eq!(entry.content().unwrap().as_ref(), raw.as_slice());
        let name = entry.path.strip_prefix(&root).unwrap();
        assert_eq!(entry.depth, name.components().count().saturating_sub(1));
    }
}

#[test]
fn registry_rejects_duplicate_keys() {
    let mut files = UFS::new();
    files
        .add("assets", Bind::Embeds(bind_dir!("examples/files")))
        .unwrap();
    assert!(
        files
            .add("assets", Bind::Statics(Statics::new("examples/files")))
            .is_err()
    );
    assert!(files.get(&"missing").is_err());
    assert!(files.get(&"assets").unwrap().embeds().is_some());
    assert!(files.get(&"assets").unwrap().statics().is_none());
}

#[test]
fn codec_sources_match() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = std::fs::read_to_string(root.join("src/compress.rs")).unwrap();
    let macros = std::fs::read_to_string(root.join("ufs-macros/src/compress.rs")).unwrap();
    let strip = |source: &str| {
        source
            .lines()
            .skip_while(|line| line.starts_with("//!") || line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(strip(&runtime), strip(&macros));
}
