//! `bind_dir!` implementation. Tokens are parsed by hand and the generated Rust
//! is spliced back onto the caller's `$crate` ident. Archive bytes are expanded
//! with the shared `src/archive.rs` when `archives = true`.

#![forbid(unsafe_code)]

use proc_macro::{Group, Ident, TokenStream, TokenTree};
use std::path::{Component, Path, PathBuf};

#[path = "../../src/archive.rs"]
mod archive;
#[path = "../../src/compress.rs"]
mod compress;

const MAX_FILES: usize = 100_000;
const MAX_DEPTH: usize = 64;
const MAX_TOTAL: u64 = 256 * 1024 * 1024;

struct Args {
    path: String,
    read: bool,
    root: bool,
    absolute: bool,
    /// `None` stores the file as-is. A level is optional; the decoder does not care which one was used.
    compress: Option<compress::Level>,
    archives: bool,
    archive_entry: bool,
}

struct Asset {
    absolute: PathBuf,
    relative: String,
    size: u64,
}

enum Payload {
    /// `include_bytes!` of the original file.
    Raw,
    /// Uncompressed bytes that are not a source file (an archive member).
    /// Emitted as a byte-string literal, not a cache file.
    Literal(Vec<u8>),
    /// ULZ1 bytes emitted as a byte-string literal.
    Compressed(Vec<u8>),
    /// Path and size only. The original file is tracked with `include_bytes!`
    /// but its bytes are not referenced, so a release build can drop them.
    Omitted,
}

struct Planned {
    relative: String,
    size: u64,
    absolute: PathBuf,
    payload: Payload,
    /// Extra file to `include_bytes!` so Cargo rebuilds when an archive changes
    /// even if only its members are embedded.
    extra_track: Option<PathBuf>,
}

/// Embed a directory. Prefer [`ufs::bind_dir`](https://docs.rs/ufs), which documents
/// every option: `read`, `root`, `absolute`, `compress` (`fast`, `balanced`, `max`),
/// `archives`, and `archive_entry`. All of them are optional. `..` is a normal path.
#[proc_macro]
pub fn bind_dir_impl(input: TokenStream) -> TokenStream {
    match expand(input) {
        Ok(stream) => stream,
        Err(message) => error_tokens(&message),
    }
}

fn expand(input: TokenStream) -> Result<TokenStream, String> {
    let (krate, rest) = split_krate(input)?;
    let args = parse_args(&rest)?;
    if args.compress.is_some() && !args.read {
        return Err("compress requires read = true".into());
    }
    if args.archives && !args.read {
        return Err("archives = true requires read = true".into());
    }
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| "CARGO_MANIFEST_DIR is not set".to_string())?;
    let manifest = PathBuf::from(manifest);
    let root = resolve_root(&manifest, &args.path, args.absolute)?;
    let assets = collect(&root, args.root, &target_dir(&manifest))?;
    if args.read {
        let total: u64 = assets.iter().map(|asset| asset.size).sum();
        if total > MAX_TOTAL {
            return Err("embedded files exceed the 256 MiB limit".into());
        }
        for asset in &assets {
            if asset.size > compress::MAX_UNCOMPRESSED as u64 {
                return Err(format!(
                    "{} exceeds the 64 MiB embed limit ({} bytes)",
                    asset.relative, asset.size
                ));
            }
        }
    }
    let planned = plan(
        &assets,
        args.read,
        args.compress,
        args.archives,
        args.archive_entry,
    )?;
    let body = render(&planned);
    splice_crate(&krate, &body)
}

fn split_krate(input: TokenStream) -> Result<(Ident, String), String> {
    let trees: Vec<TokenTree> = input.into_iter().collect();
    let marker_ok = matches!(
        trees.get(0..4),
        Some(
            [
                TokenTree::Punct(at),
                TokenTree::Ident(crate_kw),
                TokenTree::Ident(dollar_crate),
                TokenTree::Punct(semi),
            ]
        ) if at.as_char() == '@'
            && crate_kw.to_string() == "crate"
            && dollar_crate.to_string() == "$crate"
            && semi.as_char() == ';'
    );
    if !marker_ok {
        return Err("bind_dir_impl must be invoked through the ufs::bind_dir! macro".into());
    }
    let TokenTree::Ident(krate) = trees[2].clone() else {
        return Err("missing crate marker".into());
    };
    let rest: TokenStream = trees.into_iter().skip(4).collect();
    Ok((krate, rest.to_string()))
}

fn parse_args(input: &str) -> Result<Args, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("expected a directory path string".into());
    }
    let (path, mut rest) = take_string(input)?;
    if path.is_empty() {
        return Err("directory path is empty".into());
    }
    rest = rest.trim_start();
    let mut read = true;
    let mut root = false;
    let mut absolute = false;
    let mut compress = None;
    let mut archives = false;
    let mut archive_entry = true;
    let mut seen = [false; 6];
    while !rest.is_empty() {
        if !rest.starts_with(',') {
            return Err(format!("expected ',' before `{rest}`"));
        }
        rest = rest[1..].trim_start();
        if rest.is_empty() {
            break;
        }
        let (key, after_key) = take_ident(rest)?;
        rest = after_key.trim_start();
        if !rest.starts_with('=') {
            return Err(format!("expected '=' after {key}"));
        }
        rest = rest[1..].trim_start();
        let (value, after_value) = take_value(rest)?;
        rest = after_value.trim_start();
        if key == "compress" {
            if seen[3] {
                return Err("duplicate option `compress`".into());
            }
            seen[3] = true;
            compress = parse_compress(&value)?;
            continue;
        }
        let flag = match value.as_str() {
            "true" => true,
            "false" => false,
            _ => return Err(format!("expected true or false after {key}, found {value}")),
        };
        let slot = match key.as_str() {
            "read" => 0,
            "root" => 1,
            "absolute" => 2,
            "archives" => 4,
            "archive_entry" => 5,
            other => return Err(format!("unknown option `{other}`")),
        };
        if seen[slot] {
            return Err(format!("duplicate option `{key}`"));
        }
        seen[slot] = true;
        match slot {
            0 => read = flag,
            1 => root = flag,
            2 => absolute = flag,
            4 => archives = flag,
            5 => archive_entry = flag,
            _ => {}
        }
    }
    Ok(Args {
        path,
        read,
        root,
        absolute,
        compress,
        archives,
        archive_entry,
    })
}

fn take_string(input: &str) -> Result<(String, &str), String> {
    let input = input.trim_start();
    if let Some(rest) = input.strip_prefix('r') {
        return take_raw_string(input, rest);
    }
    let Some(body) = input.strip_prefix('"') else {
        return Err("expected a string literal path".into());
    };
    let mut out = String::new();
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => return Ok((out, &body[i + 1..])),
            b'\\' => {
                i += 1;
                if i >= bytes.len() {
                    return Err("unterminated string escape".into());
                }
                match bytes[i] {
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'\\' => out.push('\\'),
                    b'\'' => out.push('\''),
                    b'"' => out.push('"'),
                    b'0' => out.push('\0'),
                    b'x' => {
                        if i + 2 >= bytes.len() {
                            return Err("bad hex escape".into());
                        }
                        let hex = body.get(i + 1..i + 3).ok_or("bad hex escape")?;
                        let value = u8::from_str_radix(hex, 16).map_err(|_| "bad hex escape")?;
                        out.push(char::from(value));
                        i += 2;
                    }
                    b'u' => {
                        if bytes.get(i + 1) != Some(&b'{') {
                            return Err("bad unicode escape".into());
                        }
                        let start = i + 2;
                        let end = body[start..].find('}').ok_or("bad unicode escape")? + start;
                        if end - start == 0 || end - start > 6 {
                            return Err("bad unicode escape".into());
                        }
                        let value = u32::from_str_radix(&body[start..end], 16)
                            .map_err(|_| "bad unicode escape")?;
                        out.push(char::from_u32(value).ok_or("bad unicode escape")?);
                        i = end;
                    }
                    other => {
                        return Err(format!("unknown string escape \\{}", other as char));
                    }
                }
                i += 1;
            }
            _ => {
                let ch = body[i..].chars().next().ok_or("bad string")?;
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    Err("unterminated string literal".into())
}

fn take_raw_string<'a>(full: &'a str, after_r: &'a str) -> Result<(String, &'a str), String> {
    let hashes = after_r.bytes().take_while(|byte| *byte == b'#').count();
    let after_hashes = &after_r[hashes..];
    let Some(body) = after_hashes.strip_prefix('"') else {
        return Err("expected a string literal path".into());
    };
    let end_pat = format!("\"{}", "#".repeat(hashes));
    let end = body.find(&end_pat).ok_or("unterminated raw string")?;
    let content = body[..end].to_string();
    let consumed = full.len() - body.len() + end + end_pat.len();
    Ok((content, &full[consumed..]))
}

fn take_ident(input: &str) -> Result<(String, &str), String> {
    let input = input.trim_start();
    let mut end = 0;
    for (index, ch) in input.char_indices() {
        let ok = if index == 0 {
            ch == '_' || ch.is_ascii_alphabetic()
        } else {
            ch == '_' || ch.is_ascii_alphanumeric()
        };
        if !ok {
            break;
        }
        end = index + ch.len_utf8();
    }
    if end == 0 {
        return Err(format!("expected an identifier near `{input}`"));
    }
    Ok((input[..end].to_string(), &input[end..]))
}

fn take_value(input: &str) -> Result<(String, &str), String> {
    let input = input.trim_start();
    if input.starts_with('"') || input.starts_with('r') {
        take_string(input)
    } else {
        take_ident(input)
    }
}

fn parse_compress(value: &str) -> Result<Option<compress::Level>, String> {
    match value {
        "false" | "off" | "none" => Ok(None),
        "true" | "balanced" | "default" => Ok(Some(compress::Level::Balanced)),
        "fast" | "min" => Ok(Some(compress::Level::Fast)),
        "max" | "best" | "high" => Ok(Some(compress::Level::Max)),
        other => Err(format!(
            "unknown compress level `{other}` (expected false, fast, balanced, or max)"
        )),
    }
}

fn resolve_root(manifest: &Path, requested: &str, absolute: bool) -> Result<PathBuf, String> {
    if requested.contains('\0') {
        return Err("directory path contains a NUL byte".into());
    }
    let requested_path = Path::new(requested);
    let joined = if absolute {
        if !requested_path.is_absolute() {
            return Err("absolute = true requires an absolute path".into());
        }
        requested_path.to_path_buf()
    } else {
        manifest.join(requested_path)
    };
    let meta = std::fs::symlink_metadata(&joined)
        .map_err(|err| format!("cannot read {}: {err}", joined.display()))?;
    if meta.file_type().is_symlink() {
        return Err(format!(
            "refusing to embed symlink directory {}",
            joined.display()
        ));
    }
    if !meta.is_dir() {
        return Err(format!("{} is not a directory", joined.display()));
    }
    let root = joined
        .canonicalize()
        .map_err(|err| format!("cannot resolve {}: {err}", joined.display()))?;
    if root.parent().is_none() {
        return Err("refusing to embed the filesystem root".into());
    }
    let manifest_canon = manifest
        .canonicalize()
        .map_err(|err| format!("cannot resolve manifest dir: {err}"))?;
    if !absolute && root == manifest_canon {
        return Err(
            "refusing to embed the crate root; choose a subdirectory, or pass absolute = true"
                .into(),
        );
    }
    let target = canonical_if_exists(&target_dir(manifest));
    if root.starts_with(&target) {
        return Err("refusing to embed the Cargo target directory".into());
    }
    Ok(root)
}

fn collect(root: &Path, with_root: bool, target: &Path) -> Result<Vec<Asset>, String> {
    let target = canonical_if_exists(target);
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let depth = dir
            .strip_prefix(root)
            .map(|rel| rel.components().count())
            .unwrap_or(0);
        if depth > MAX_DEPTH {
            return Err("directory nests deeper than 64 levels".into());
        }
        let reader = std::fs::read_dir(&dir)
            .map_err(|err| format!("cannot read {}: {err}", dir.display()))?;
        for entry in reader {
            let entry = entry.map_err(|err| format!("cannot read {}: {err}", dir.display()))?;
            let file_type = entry
                .file_type()
                .map_err(|err| format!("cannot stat {}: {err}", entry.path().display()))?;
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if !path.starts_with(root) || path.starts_with(&target) {
                if path.starts_with(&target) {
                    continue;
                }
                return Err(format!("{} is outside {}", path.display(), root.display()));
            }
            if file_type.is_dir() {
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            if files.len() >= MAX_FILES {
                return Err("directory has more than 100000 files".into());
            }
            let meta = entry
                .metadata()
                .map_err(|err| format!("cannot stat {}: {err}", path.display()))?;
            let relative = relative_path(root, &path, with_root)?;
            files.push(Asset {
                absolute: path,
                relative,
                size: meta.len(),
            });
        }
    }
    files.sort_by(|a, b| a.relative.cmp(&b.relative));
    Ok(files)
}

fn relative_path(root: &Path, file: &Path, with_root: bool) -> Result<String, String> {
    let rel = file
        .strip_prefix(root)
        .map_err(|_| format!("{} is outside {}", file.display(), root.display()))?;
    let mut parts = Vec::new();
    if with_root {
        let name = root
            .file_name()
            .ok_or("embed root has no file name")?
            .to_str()
            .ok_or("embed root name is not utf-8")?;
        if name.is_empty() || name == "." || name == ".." {
            return Err("embed root has an unusable name".into());
        }
        parts.push(name.to_string());
    }
    for component in rel.components() {
        match component {
            Component::Normal(part) => {
                let part = part.to_str().ok_or("file path is not utf-8")?;
                if part.is_empty() || part == "." || part == ".." {
                    return Err(format!("refusing path component `{part}`"));
                }
                parts.push(part.to_string());
            }
            _ => return Err("refusing a non-normal path component".into()),
        }
    }
    if parts.is_empty() {
        return Err("empty relative path".into());
    }
    Ok(parts.join("/"))
}

fn plan(
    assets: &[Asset],
    read: bool,
    compress: Option<compress::Level>,
    archives: bool,
    archive_entry: bool,
) -> Result<Vec<Planned>, String> {
    let mut planned = Vec::with_capacity(assets.len());
    let mut pending = Vec::new();
    let mut total = 0u64;
    for asset in assets {
        if !read {
            planned.push(Planned {
                relative: asset.relative.clone(),
                size: asset.size,
                absolute: asset.absolute.clone(),
                payload: Payload::Omitted,
                extra_track: None,
            });
            continue;
        }
        let bytes = read_regular(&asset.absolute)?;
        let looks_archive =
            archive::is_archive_name(&asset.relative) || archive::is_archive_bytes(&bytes);
        if archives && looks_archive {
            let nodes = archive::expand(&bytes, &asset.relative, archive_entry)?;
            if archive_entry {
                queue(
                    &mut pending,
                    &mut total,
                    &asset.relative,
                    bytes,
                    Some(asset.absolute.clone()),
                    None,
                )?;
            }
            for node in nodes {
                let relative = format!("{}/{}", asset.relative, node.path);
                let track = if archive_entry {
                    None
                } else {
                    Some(asset.absolute.clone())
                };
                queue(&mut pending, &mut total, &relative, node.bytes, None, track)?;
            }
            continue;
        }
        queue(
            &mut pending,
            &mut total,
            &asset.relative,
            bytes,
            Some(asset.absolute.clone()),
            None,
        )?;
    }
    planned.extend(finish(compress, pending)?);
    Ok(planned)
}

struct Pending {
    relative: String,
    bytes: Vec<u8>,
    original: Option<PathBuf>,
    extra_track: Option<PathBuf>,
}

fn queue(
    pending: &mut Vec<Pending>,
    total: &mut u64,
    relative: &str,
    bytes: Vec<u8>,
    original: Option<PathBuf>,
    extra_track: Option<PathBuf>,
) -> Result<(), String> {
    if bytes.len() as u64 > compress::MAX_UNCOMPRESSED as u64 {
        return Err(format!(
            "{relative} exceeds the 64 MiB embed limit ({} bytes)",
            bytes.len()
        ));
    }
    *total = total.saturating_add(bytes.len() as u64);
    if *total > MAX_TOTAL {
        return Err("embedded files exceed the 256 MiB limit".into());
    }
    pending.push(Pending {
        relative: relative.to_string(),
        bytes,
        original,
        extra_track,
    });
    Ok(())
}

fn finish(level: Option<compress::Level>, pending: Vec<Pending>) -> Result<Vec<Planned>, String> {
    let mut blobs = if let Some(level) = level {
        let inputs: Vec<&[u8]> = pending.iter().map(|item| item.bytes.as_slice()).collect();
        Some(compress::compress_many(&inputs, level).map_err(str::to_string)?)
    } else {
        None
    };
    let mut planned = Vec::with_capacity(pending.len());
    for (index, item) in pending.into_iter().enumerate() {
        if let Some(blobs) = blobs.as_mut() {
            let blob = std::mem::take(&mut blobs[index]);
            if blob.len() < item.bytes.len() {
                planned.push(Planned {
                    relative: item.relative,
                    size: item.bytes.len() as u64,
                    absolute: item.original.unwrap_or_default(),
                    payload: Payload::Compressed(blob),
                    extra_track: item.extra_track,
                });
                continue;
            }
        }
        if let Some(path) = item.original {
            planned.push(Planned {
                relative: item.relative,
                size: item.bytes.len() as u64,
                absolute: path,
                payload: Payload::Raw,
                extra_track: item.extra_track,
            });
        } else {
            planned.push(Planned {
                relative: item.relative,
                size: item.bytes.len() as u64,
                absolute: PathBuf::new(),
                payload: Payload::Literal(item.bytes),
                extra_track: item.extra_track,
            });
        }
    }
    Ok(planned)
}

fn read_regular(path: &Path) -> Result<Vec<u8>, String> {
    let meta = std::fs::symlink_metadata(path)
        .map_err(|err| format!("cannot stat {}: {err}", path.display()))?;
    if !meta.file_type().is_file() {
        return Err(format!("{} is not a regular file", path.display()));
    }
    if meta.len() > compress::MAX_UNCOMPRESSED as u64 {
        return Err(format!(
            "{} exceeds the 64 MiB embed limit ({} bytes)",
            path.display(),
            meta.len()
        ));
    }
    std::fs::read(path).map_err(|err| format!("cannot read {}: {err}", path.display()))
}

fn target_dir(manifest: &Path) -> PathBuf {
    if let Some(dir) = std::env::var_os("CARGO_TARGET_DIR") {
        return PathBuf::from(dir);
    }
    canonical_if_exists(manifest).join("target")
}

fn canonical_if_exists(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn render(planned: &[Planned]) -> String {
    let mut body = String::from("{\n");
    let mut seen: Vec<&Path> = Vec::new();
    for (index, asset) in planned.iter().enumerate() {
        let track = match &asset.payload {
            Payload::Raw => None,
            Payload::Omitted => Some(asset.absolute.as_path()),
            Payload::Literal(_) | Payload::Compressed(_) => asset
                .extra_track
                .as_deref()
                .or(Some(asset.absolute.as_path())),
        };
        let Some(path) = track else {
            continue;
        };
        if path.as_os_str().is_empty() || seen.contains(&path) {
            continue;
        }
        seen.push(path);
        let lit = rust_string(&path_lit(path));
        body.push_str(&format!(
            "#[allow(dead_code)]\nconst _UFS_TRACK_{index}: &[u8] = include_bytes!({lit});\n"
        ));
    }
    body.push_str("__UFS_CRATE__::UFS::from_entries(vec![\n");
    for asset in planned {
        let rel = rust_string(&asset.relative);
        match &asset.payload {
            Payload::Raw => {
                let lit = rust_string(&path_lit(&asset.absolute));
                body.push_str(&format!(
                    "__UFS_CRATE__::Entry::embedded({rel}, include_bytes!({lit})),\n"
                ));
            }
            Payload::Literal(bytes) => {
                body.push_str(&format!(
                    "__UFS_CRATE__::Entry::embedded({rel}, {}),\n",
                    rust_byte_str(bytes)
                ));
            }
            Payload::Compressed(bytes) => {
                body.push_str(&format!(
                    "__UFS_CRATE__::Entry::compressed({rel}, {}u64, {}),\n",
                    asset.size,
                    rust_byte_str(bytes)
                ));
            }
            Payload::Omitted => {
                body.push_str(&format!(
                    "__UFS_CRATE__::Entry::omitted({rel}, {}u64),\n",
                    asset.size
                ));
            }
        }
    }
    body.push_str("])\n}");
    body
}

/// Byte-string literal. The result is `&'static [u8]` with no runtime copy.
fn rust_byte_str(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().saturating_mul(4).saturating_add(3));
    out.push_str("b\"");
    for &byte in bytes {
        match byte {
            b'\\' | b'"' => {
                out.push('\\');
                out.push(byte as char);
            }
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7e => out.push(byte as char),
            _ => {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                out.push_str("\\x");
                out.push(HEX[(byte >> 4) as usize] as char);
                out.push(HEX[(byte & 0xf) as usize] as char);
            }
        }
    }
    out.push('"');
    out
}

fn path_lit(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn rust_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn splice_crate(krate: &Ident, body: &str) -> Result<TokenStream, String> {
    let parsed: TokenStream = body
        .parse()
        .map_err(|_| format!("internal codegen error near {body}"))?;
    Ok(replace_crate(krate, parsed))
}

fn replace_crate(krate: &Ident, input: TokenStream) -> TokenStream {
    input
        .into_iter()
        .map(|tree| match tree {
            TokenTree::Ident(id) if id.to_string() == "__UFS_CRATE__" => {
                TokenTree::Ident(krate.clone())
            }
            TokenTree::Group(group) => {
                let inner = replace_crate(krate, group.stream());
                TokenTree::Group(Group::new(group.delimiter(), inner))
            }
            other => other,
        })
        .collect()
}

fn error_tokens(message: &str) -> TokenStream {
    let lit = rust_string(message);
    format!("::core::compile_error!({lit})")
        .parse()
        .unwrap_or_else(|_| {
            "compile_error!(\"bind_dir failed\")"
                .parse()
                .expect("static compile_error token")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_options() {
        let args = parse_args(r#""examples/files""#).unwrap();
        assert_eq!(args.path, "examples/files");
        assert!(args.read);
        assert!(args.compress.is_none());

        let args = parse_args(r#""assets", compress = true, root = true, read = false,"#).unwrap();
        assert_eq!(args.compress, Some(compress::Level::Balanced));
        assert!(args.root && !args.read);

        let args = parse_args(r#""assets", compress = fast"#).unwrap();
        assert_eq!(args.compress, Some(compress::Level::Fast));
        assert!(parse_args(r#""assets", compress = "max", compress = max"#).is_err());
        let args = parse_args(r#""assets", compress = "best""#).unwrap();
        assert_eq!(args.compress, Some(compress::Level::Max));
        let args = parse_args(r#""assets", compress = false"#).unwrap();
        assert!(args.compress.is_none());
        assert!(parse_args(r#""a", compress = nope"#).is_err());

        let args = parse_args(r#"r"examples/files""#).unwrap();
        assert_eq!(args.path, "examples/files");

        let args = parse_args(r##"r#"examples/files"#"##).unwrap();
        assert_eq!(args.path, "examples/files");

        assert!(parse_args(r#""a", nope = true"#).is_err());
        assert!(parse_args(r#""a", read = false, read = true"#).is_err());
        assert!(parse_args("").is_err());
    }

    #[test]
    fn unescapes_paths() {
        let (value, rest) = take_string("\"a\\\\b\\\"c\", next").unwrap();
        assert_eq!(value, "a\\b\"c");
        assert!(rest.trim_start().starts_with(','));
    }

    #[test]
    fn skips_symlinks_and_rejects_escape() {
        let root = std::env::temp_dir().join(format!("ufs-macro-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let manifest = root.join("crate");
        let assets = manifest.join("assets");
        let outside = root.join("outside");
        std::fs::create_dir_all(&assets).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(assets.join("keep.txt"), b"keep").unwrap();
        std::fs::write(outside.join("secret.txt"), b"secret").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.join("secret.txt"), assets.join("leak.txt")).unwrap();

        let resolved = resolve_root(&manifest, "assets", false).unwrap();
        let found = collect(&resolved, false, &manifest.join("target")).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].relative, "keep.txt");

        let escaped = resolve_root(&manifest, "../outside", false).unwrap();
        assert_eq!(escaped, outside.canonicalize().unwrap());
        let shown = collect(&escaped, true, &manifest.join("target")).unwrap();
        assert_eq!(shown[0].relative, "outside/secret.txt");
        assert_eq!(
            collect(&escaped, false, &manifest.join("target")).unwrap()[0].relative,
            "secret.txt"
        );

        let with_root = collect(&resolved, true, &manifest.join("target")).unwrap();
        assert_eq!(with_root[0].relative, "assets/keep.txt");

        let abs = resolved.to_str().unwrap();
        let via_absolute = resolve_root(&manifest, abs, true).unwrap();
        assert_eq!(via_absolute, resolved);
        assert!(resolve_root(&manifest, "assets", true).is_err());

        let _ = std::fs::remove_dir_all(&root);
    }
}
