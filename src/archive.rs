//! Read zip, tar, tar.gz, tar.xz, and 7z archives.
//!
//! Members are materialized only after their declared size is checked.
//! Paths that escape the archive (`..`, absolute paths, NUL) are rejected.
//! Symlinks and other non-regular tar entries are skipped, not followed.

#![forbid(unsafe_code)]

use std::io::{self, Cursor, Read, Write};

/// Largest single member this crate will allocate.
pub const MAX_MEMBER: u64 = 64 * 1024 * 1024;
/// Largest total uncompressed payload of one expand.
pub const MAX_TOTAL: u64 = 256 * 1024 * 1024;
/// Most members one expand will return.
pub const MAX_FILES: usize = 100_000;
/// Nested archives opened while expanding.
pub const MAX_NEST: u32 = 8;

/// One regular file pulled out of an archive.
#[derive(Debug, Clone)]
pub struct Node {
    /// Path relative to the archive that was passed to [`expand`], using `/`.
    pub path: String,
    pub bytes: Vec<u8>,
}

/// `true` when the file name uses a supported archive extension.
pub fn is_archive_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.ends_with(".zip")
        || name.ends_with(".tar")
        || name.ends_with(".tgz")
        || name.ends_with(".tar.gz")
        || name.ends_with(".txz")
        || name.ends_with(".tar.xz")
        || name.ends_with(".7z")
}

/// `true` when `bytes` starts with a zip, gzip, xz, 7z, or ustar header.
pub fn is_archive_bytes(bytes: &[u8]) -> bool {
    detect(bytes, "").is_ok()
}

/// Expand `bytes` into regular files.
///
/// Nested archives are expanded too, up to [`MAX_NEST`]. When `archive_entry`
/// is set, each nested archive is also returned as its own node (the bytes of
/// that archive) in addition to its children. The top-level archive is not
/// included; the caller already has it.
pub fn expand(bytes: &[u8], hint: &str, archive_entry: bool) -> Result<Vec<Node>, String> {
    let mut out = Vec::new();
    let mut total = 0u64;
    expand_into(bytes, hint, "", archive_entry, 0, &mut total, &mut out)?;
    Ok(out)
}

fn expand_into(
    bytes: &[u8],
    hint: &str,
    prefix: &str,
    archive_entry: bool,
    depth: u32,
    total: &mut u64,
    out: &mut Vec<Node>,
) -> Result<(), String> {
    if depth > MAX_NEST {
        return Err("archive nesting exceeds 8 levels".into());
    }
    let members = read_one(bytes, hint, total)?;
    for member in members {
        let path = join_path(prefix, &member.path);
        let nested = depth < MAX_NEST && is_archive_bytes(&member.bytes);
        if nested {
            expand_into(
                &member.bytes,
                &path,
                &path,
                archive_entry,
                depth + 1,
                total,
                out,
            )?;
            if archive_entry {
                out.push(Node {
                    path,
                    bytes: member.bytes,
                });
                if out.len() > MAX_FILES {
                    return Err("archive has more than 100000 files".into());
                }
            }
        } else {
            out.push(Node {
                path,
                bytes: member.bytes,
            });
            if out.len() > MAX_FILES {
                return Err("archive has more than 100000 files".into());
            }
        }
    }
    Ok(())
}

struct Member {
    path: String,
    bytes: Vec<u8>,
}

enum Kind {
    Zip,
    Tar,
    TarGz,
    TarXz,
    SevenZ,
}

fn detect(bytes: &[u8], hint: &str) -> Result<Kind, String> {
    if bytes.starts_with(b"PK\x03\x04")
        || bytes.starts_with(b"PK\x05\x06")
        || bytes.starts_with(b"PK\x07\x08")
    {
        return Ok(Kind::Zip);
    }
    if bytes.starts_with(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) {
        return Ok(Kind::SevenZ);
    }
    if bytes.starts_with(&[0xFD, b'7', b'z', b'X', b'Z', 0x00]) {
        return Ok(Kind::TarXz);
    }
    if bytes.starts_with(&[0x1F, 0x8B]) {
        return Ok(Kind::TarGz);
    }
    if bytes.len() > 262 && &bytes[257..262] == b"ustar" {
        return Ok(Kind::Tar);
    }
    let hint = hint.to_ascii_lowercase();
    if hint.ends_with(".tar.gz") || hint.ends_with(".tgz") {
        return Ok(Kind::TarGz);
    }
    if hint.ends_with(".tar.xz") || hint.ends_with(".txz") {
        return Ok(Kind::TarXz);
    }
    if hint.ends_with(".tar") {
        return Ok(Kind::Tar);
    }
    if hint.ends_with(".zip") {
        return Ok(Kind::Zip);
    }
    if hint.ends_with(".7z") {
        return Ok(Kind::SevenZ);
    }
    Err("not a zip, tar, tar.gz, tar.xz, or 7z archive".into())
}

fn read_one(bytes: &[u8], hint: &str, total: &mut u64) -> Result<Vec<Member>, String> {
    if bytes.len() as u64 > MAX_MEMBER {
        return Err("archive exceeds the 64 MiB compressed-size limit".into());
    }
    match detect(bytes, hint)? {
        Kind::Zip => read_zip(bytes, total),
        Kind::Tar => read_tar(Cursor::new(bytes), total),
        Kind::TarGz => {
            let decoder = flate2::read::GzDecoder::new(Cursor::new(bytes));
            read_tar(Limited::new(decoder, MAX_TOTAL), total)
        }
        Kind::TarXz => {
            let mut decoded = CapWriter::new(MAX_TOTAL as usize);
            lzma_rs::xz_decompress(&mut &bytes[..], &mut decoded).map_err(|err| err.to_string())?;
            read_tar(Cursor::new(decoded.into_vec()), total)
        }
        Kind::SevenZ => read_7z(bytes, total),
    }
}

fn read_zip(bytes: &[u8], total: &mut u64) -> Result<Vec<Member>, String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|err| err.to_string())?;
    if archive.len() > MAX_FILES {
        return Err("archive has more than 100000 files".into());
    }
    let mut members = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|err| err.to_string())?;
        if file.is_dir() {
            continue;
        }
        let Some(name) = file.enclosed_name() else {
            return Err("archive member path escapes the archive".into());
        };
        let path = sanitize(&name.to_string_lossy())?;
        let declared = file.size();
        account(total, declared)?;
        let bytes = read_declared(&mut file, declared)?;
        members.push(Member { path, bytes });
    }
    Ok(members)
}

fn read_tar<R: Read>(reader: R, total: &mut u64) -> Result<Vec<Member>, String> {
    let mut archive = tar::Archive::new(reader);
    archive.set_ignore_zeros(true);
    let mut members = Vec::new();
    for item in archive.entries().map_err(|err| err.to_string())? {
        let mut item = item.map_err(|err| err.to_string())?;
        let kind = item.header().entry_type();
        if kind.is_dir() {
            continue;
        }
        // Symlinks, hard links, and device nodes are not followed.
        if !kind.is_file() {
            continue;
        }
        let raw = item.path().map_err(|err| err.to_string())?;
        let path = sanitize(&raw.to_string_lossy())?;
        let declared = item.size();
        account(total, declared)?;
        let bytes = read_declared(&mut item, declared)?;
        members.push(Member { path, bytes });
        if members.len() > MAX_FILES {
            return Err("archive has more than 100000 files".into());
        }
    }
    Ok(members)
}

fn read_7z(bytes: &[u8], total: &mut u64) -> Result<Vec<Member>, String> {
    let mut reader =
        sevenz_rust2::ArchiveReader::new(Cursor::new(bytes), sevenz_rust2::Password::empty())
            .map_err(|err| err.to_string())?;
    let mut members = Vec::new();
    let mut failure: Option<String> = None;
    reader
        .for_each_entries(|entry, stream| {
            if failure.is_some() {
                return Ok(false);
            }
            if entry.is_directory() || entry.is_anti_item() || !entry.has_stream() {
                if entry.is_anti_item() {
                    failure = Some("7z anti-item is not allowed".into());
                    return Ok(false);
                }
                return Ok(true);
            }
            let path = match sanitize(entry.name()) {
                Ok(path) => path,
                Err(err) => {
                    failure = Some(err);
                    return Ok(false);
                }
            };
            let declared = entry.size();
            if let Err(err) = account(total, declared) {
                failure = Some(err);
                return Ok(false);
            }
            match read_declared(stream, declared) {
                Ok(bytes) => members.push(Member { path, bytes }),
                Err(err) => failure = Some(err),
            }
            if members.len() > MAX_FILES {
                failure = Some("archive has more than 100000 files".into());
                return Ok(false);
            }
            Ok(true)
        })
        .map_err(|err| err.to_string())?;
    if let Some(err) = failure {
        return Err(err);
    }
    Ok(members)
}

fn sanitize(name: &str) -> Result<String, String> {
    if name.contains('\0') {
        return Err("archive member path contains NUL".into());
    }
    let mut out = String::new();
    for part in name.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err("archive member path escapes the archive".into());
        }
        if !out.is_empty() {
            out.push('/');
        }
        out.push_str(part);
    }
    if out.is_empty() {
        return Err("archive member path is empty".into());
    }
    Ok(out)
}

fn join_path(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}/{name}")
    }
}

fn account(total: &mut u64, bytes: u64) -> Result<(), String> {
    if bytes > MAX_MEMBER {
        return Err("archive member exceeds the 64 MiB limit".into());
    }
    *total = total.saturating_add(bytes);
    if *total > MAX_TOTAL {
        return Err("archive expands past 256 MiB".into());
    }
    Ok(())
}

fn read_declared(reader: &mut dyn Read, declared: u64) -> Result<Vec<u8>, String> {
    if declared > MAX_MEMBER {
        return Err("archive member exceeds the 64 MiB limit".into());
    }
    let mut buf = Vec::new();
    buf.try_reserve(declared as usize)
        .map_err(|_| "out of memory".to_string())?;
    reader
        .take(declared)
        .read_to_end(&mut buf)
        .map_err(|err| err.to_string())?;
    if buf.len() as u64 != declared {
        return Err("truncated archive member".into());
    }
    Ok(buf)
}

struct Limited<R> {
    inner: R,
    left: u64,
}

impl<R> Limited<R> {
    fn new(inner: R, limit: u64) -> Self {
        Self { inner, left: limit }
    }
}

impl<R: Read> Read for Limited<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.left == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "archive expands past 256 MiB",
            ));
        }
        let max = usize::try_from(self.left)
            .unwrap_or(usize::MAX)
            .min(buf.len());
        let read = self.inner.read(&mut buf[..max])?;
        self.left -= read as u64;
        Ok(read)
    }
}

struct CapWriter {
    buf: Vec<u8>,
    limit: usize,
}

impl CapWriter {
    fn new(limit: usize) -> Self {
        Self {
            buf: Vec::new(),
            limit,
        }
    }

    fn into_vec(self) -> Vec<u8> {
        self.buf
    }
}

impl Write for CapWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if self.buf.len().saturating_add(data.len()) > self.limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "archive expands past 256 MiB",
            ));
        }
        self.buf.extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zip_bytes(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            for (name, data) in files {
                writer
                    .start_file(
                        *name,
                        zip::write::SimpleFileOptions::default()
                            .compression_method(zip::CompressionMethod::Deflated),
                    )
                    .unwrap();
                writer.write_all(data).unwrap();
            }
            writer.finish().unwrap();
        }
        cursor.into_inner()
    }

    fn tar_bytes(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        for (name, data) in files {
            let mut header = tar::Header::new_gnu();
            header.set_path(*name).unwrap();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append(&header, *data).unwrap();
        }
        builder.into_inner().unwrap()
    }

    #[test]
    fn reads_zip_tar_tgz_txz() {
        let files = [("dir/hello.txt", &b"hello"[..]), ("solo.txt", &b"solo"[..])];
        for (label, bytes) in [
            ("pack.zip", zip_bytes(&files)),
            ("pack.tar", tar_bytes(&files)),
            ("pack.tgz", gzip(&tar_bytes(&files))),
            ("pack.txz", xz(&tar_bytes(&files))),
        ] {
            let nodes = expand(&bytes, label, false).unwrap();
            assert_eq!(nodes.len(), 2, "{label}");
            let hello = nodes.iter().find(|n| n.path == "dir/hello.txt").unwrap();
            assert_eq!(hello.bytes, b"hello");
        }
    }

    #[test]
    fn reads_7z() {
        let dir = std::env::temp_dir().join(format!("ufs-7z-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let src = dir.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("hello.txt"), b"hello-7z").unwrap();
        let archive = dir.join("pack.7z");
        let mut writer = sevenz_rust2::ArchiveWriter::create(&archive).unwrap();
        writer.push_source_path(&src, |_| true).unwrap();
        writer.finish().unwrap();
        let bytes = std::fs::read(&archive).unwrap();
        let nodes = expand(&bytes, "pack.7z", false).unwrap();
        assert!(
            nodes.iter().any(|n| n.bytes == b"hello-7z"),
            "{:?}",
            nodes.iter().map(|n| &n.path).collect::<Vec<_>>()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_zip_slip_and_skips_tar_symlink() {
        let hostile = zip_bytes(&[("../secret.txt", b"nope")]);
        assert!(expand(&hostile, "bad.zip", false).is_err());

        let mut builder = tar::Builder::new(Vec::new());
        let mut link = tar::Header::new_gnu();
        link.set_entry_type(tar::EntryType::Symlink);
        link.set_path("link.txt").unwrap();
        link.set_link_name("/etc/passwd").unwrap();
        link.set_size(0);
        link.set_cksum();
        builder.append(&link, std::io::empty()).unwrap();
        let mut file = tar::Header::new_gnu();
        file.set_path("ok.txt").unwrap();
        file.set_size(2);
        file.set_cksum();
        builder.append(&file, &b"ok"[..]).unwrap();
        let bytes = builder.into_inner().unwrap();
        let nodes = expand(&bytes, "links.tar", false).unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].path, "ok.txt");
    }

    #[test]
    fn rejects_huge_declared_tar_size() {
        let mut header = tar::Header::new_gnu();
        header.set_path("big.txt").unwrap();
        header.set_size(MAX_MEMBER + 1);
        header.set_cksum();
        let mut builder = tar::Builder::new(Vec::new());
        builder.append(&header, std::io::repeat(0).take(0)).unwrap();
        let bytes = builder.into_inner().unwrap();
        assert!(expand(&bytes, "big.tar", false).is_err());
    }

    #[test]
    fn archive_entry_keeps_nested_archive() {
        let inner = zip_bytes(&[("note.txt", b"note")]);
        let outer = zip_bytes(&[("inner.zip", &inner), ("side.txt", b"side")]);
        let flat = expand(&outer, "outer.zip", false).unwrap();
        assert!(flat.iter().any(|n| n.path == "inner.zip/note.txt"));
        assert!(!flat.iter().any(|n| n.path == "inner.zip"));
        let kept = expand(&outer, "outer.zip", true).unwrap();
        assert!(kept.iter().any(|n| n.path == "inner.zip"));
        assert!(kept.iter().any(|n| n.path == "inner.zip/note.txt"));
        assert!(kept.iter().any(|n| n.path == "side.txt"));
    }

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    fn xz(bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        lzma_rs::xz_compress(&mut &bytes[..], &mut out).unwrap();
        out
    }
}
