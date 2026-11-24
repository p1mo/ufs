use std::path::Path;

use ufs::{Statics, UFS, bind_dir};

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
fn disk_and_embed_are_both_ufs() {
    let embedded: UFS = bind_dir!("examples/files");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/files");
    let disk: UFS = UFS::statics(&root).unwrap();
    assert_eq!(embedded.len(), disk.len());
    assert!(disk.iter().all(|entry| entry.is_local()));
    let bytes = embedded.read_all().unwrap();
    assert_eq!(bytes.len(), embedded.len());
}

#[test]
fn archives_expand_and_can_stay_entries() {
    let kept = bind_dir!("examples/archives", archives = true, archive_entry = true);
    let zip = kept.get("pack.zip").expect("archive kept as an entry");
    assert!(zip.is_archive());
    let inside = zip.archive().unwrap();
    assert_eq!(
        inside
            .get("pack.zip/hello.txt")
            .unwrap()
            .content()
            .unwrap()
            .as_ref(),
        b"hello-zip"
    );
    assert_eq!(
        kept.get("pack.zip/hello.txt")
            .unwrap()
            .content()
            .unwrap()
            .as_ref(),
        b"hello-zip"
    );
    assert_eq!(
        kept.get("pack.tar/hello.txt")
            .unwrap()
            .content()
            .unwrap()
            .as_ref(),
        b"hello-tar"
    );
    assert_eq!(
        kept.get("pack.tgz/hello.txt")
            .unwrap()
            .content()
            .unwrap()
            .as_ref(),
        b"hello-tgz"
    );
    assert_eq!(
        kept.get("pack.txz/hello.txt")
            .unwrap()
            .content()
            .unwrap()
            .as_ref(),
        b"hello-txz"
    );

    let members = bind_dir!("examples/archives", archives = true, archive_entry = false);
    assert!(members.get("pack.zip").is_none());
    assert!(members.get("pack.tar").is_none());
    assert!(members.get("pack.zip/hello.txt").is_some());

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/archives");
    let walked = Statics::new(&root)
        .archives(true)
        .archive_entry(false)
        .files()
        .unwrap();
    assert!(walked.iter().all(|entry| entry.filename != "pack.zip"));
    assert!(
        walked
            .iter()
            .any(|entry| entry.path.ends_with("pack.zip/hello.txt"))
    );
    let kept_walk = Statics::new(&root).archives(true).files().unwrap();
    assert!(
        kept_walk
            .iter()
            .any(|entry| entry.filename == "pack.zip" && entry.is_archive())
    );
}

#[test]
fn compress_levels_show_root_and_parent_paths() {
    let fast = bind_dir!("examples/../examples/files", root = true, compress = fast);
    let max = bind_dir!("examples/files", root = true, compress = "max");
    let off = bind_dir!("examples/files", root = true, compress = off);
    let disk = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/files/nested/big-demo.txt"),
    )
    .unwrap();

    for (label, set) in [("fast", &fast), ("max", &max), ("off", &off)] {
        let entry = set.get("files/nested/big-demo.txt").unwrap();
        assert_eq!(
            entry.content().unwrap().as_ref(),
            disk.as_slice(),
            "{label}"
        );
        assert_eq!(entry.is_compressed(), label != "off", "{label}");
    }
    assert!(fast.get("files/demo.txt").unwrap().content().is_ok());
}
