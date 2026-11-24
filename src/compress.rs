//! ULZ1, a small LZ77 codec. The proc-macro crate includes this file directly.
//!
//! ```text
//! b"ULZ1" | raw_len:u32le | tokens
//!   0x00..=0x7F  literal run, count = byte + 1, followed by that many bytes
//!   0x80..=0xFF  match, length = (byte & 0x7F) + 4 (4..=131),
//!                followed by distance:u16le (1..=65535)
//! ```
//!
//! Matches may overlap (`distance < length`). The decoder copies those one
//! byte at a time. Longer matches are copied as a block. Both paths produce
//! the same bytes.

/// Largest payload `decompress` will allocate.
pub const MAX_UNCOMPRESSED: usize = 64 * 1024 * 1024;

const MAGIC: &[u8; 4] = b"ULZ1";
const MIN_MATCH: usize = 4;
const MAX_MATCH: usize = 131;
const MAX_DISTANCE: usize = 65_535;
const MAX_LITERAL: usize = 128;

/// How hard [`compress_with`] searches for matches.
///
/// Every level writes the same ULZ1 stream. [`decompress`] does not take a
/// level. `Entry::content` decompresses automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// First hash-chain hit only.
    Fast,
    /// Default effort. `compress = true` selects this.
    Balanced,
    /// Longer hash-chain search.
    Max,
}

impl Level {
    fn chain_limit(self) -> usize {
        match self {
            Level::Fast => 1,
            Level::Balanced => 32,
            Level::Max => 256,
        }
    }
}

/// Compress `input` with [`Level::Balanced`].
///
/// Returns an error when `input` is larger than [`MAX_UNCOMPRESSED`].
pub fn compress(input: &[u8]) -> Result<Vec<u8>, &'static str> {
    compress_with(input, Level::Balanced)
}

/// Compress `input` at `level` into a self-delimiting ULZ1 blob.
pub fn compress_with(input: &[u8], level: Level) -> Result<Vec<u8>, &'static str> {
    if input.len() > MAX_UNCOMPRESSED {
        return Err("input exceeds compression limit");
    }
    let mut out = Vec::with_capacity(input.len() / 2 + 8);
    out.extend_from_slice(MAGIC);
    let len = u32::try_from(input.len()).map_err(|_| "input exceeds compression limit")?;
    out.extend_from_slice(&len.to_le_bytes());

    let mut head = vec![0u32; 65_536];
    let mut prev = vec![0u32; input.len()];
    let mut i = 0;
    let mut literal_start = 0;

    while i < input.len() {
        if i + MIN_MATCH <= input.len() {
            let (match_len, match_dist) = find_match(input, i, &head, &prev, level.chain_limit());
            if match_len >= MIN_MATCH {
                if literal_start < i {
                    emit_literals(&mut out, &input[literal_start..i]);
                }
                emit_match(&mut out, match_len, match_dist);
                let end = i + match_len;
                while i < end {
                    insert(input, i, &mut head, &mut prev);
                    i += 1;
                }
                literal_start = i;
                continue;
            }
        }
        insert(input, i, &mut head, &mut prev);
        i += 1;
        if i - literal_start == MAX_LITERAL {
            emit_literals(&mut out, &input[literal_start..i]);
            literal_start = i;
        }
    }
    if literal_start < i {
        emit_literals(&mut out, &input[literal_start..i]);
    }
    Ok(out)
}

/// Decode a ULZ1 blob.
///
/// Refuses to allocate more than [`MAX_UNCOMPRESSED`] bytes, and returns an
/// error instead of panicking on a truncated or inconsistent stream.
#[allow(dead_code)] // the proc-macro crate compresses and does not decode
pub fn decompress(input: &[u8]) -> Result<Vec<u8>, &'static str> {
    let raw_len = payload_len(input)?;
    let mut out = Vec::new();
    out.try_reserve(raw_len).map_err(|_| "out of memory")?;
    let mut i = 8;
    while out.len() < raw_len {
        let ctrl = *input.get(i).ok_or("truncated stream")?;
        i += 1;
        if ctrl < 0x80 {
            let n = ctrl as usize + 1;
            if out.len() + n > raw_len {
                return Err("literal exceeds declared size");
            }
            let end = i + n;
            let bytes = input.get(i..end).ok_or("truncated literal")?;
            out.extend_from_slice(bytes);
            i = end;
        } else {
            let n = (ctrl & 0x7F) as usize + MIN_MATCH;
            let dist_bytes = input.get(i..i + 2).ok_or("truncated match")?;
            let dist = u16::from_le_bytes([dist_bytes[0], dist_bytes[1]]) as usize;
            i += 2;
            if dist == 0 || dist > out.len() || dist > MAX_DISTANCE {
                return Err("bad match distance");
            }
            if out.len() + n > raw_len {
                return Err("match exceeds declared size");
            }
            copy_match(&mut out, dist, n);
        }
    }
    if i != input.len() {
        return Err("trailing bytes");
    }
    if out.len() != raw_len {
        return Err("length mismatch");
    }
    Ok(out)
}

/// Declared uncompressed length, without allocating the payload.
pub(crate) fn payload_len(input: &[u8]) -> Result<usize, &'static str> {
    if input.len() < 8 || input.get(..4) != Some(MAGIC) {
        return Err("not ULZ1");
    }
    let len_bytes: [u8; 4] = input
        .get(4..8)
        .ok_or("truncated header")?
        .try_into()
        .map_err(|_| "truncated header")?;
    let raw_len = u32::from_le_bytes(len_bytes) as usize;
    if raw_len > MAX_UNCOMPRESSED {
        return Err("uncompressed size exceeds limit");
    }
    Ok(raw_len)
}

/// Copy `n` bytes from `dist` back. Overlapping runs are copied one byte at a
/// time so a distance shorter than the match still repeats the source bytes.
fn copy_match(out: &mut Vec<u8>, dist: usize, n: usize) {
    let start = out.len() - dist;
    if dist >= n {
        let dest = out.len();
        out.resize(dest + n, 0);
        out.copy_within(start..start + n, dest);
        return;
    }
    for _ in 0..n {
        let byte = out[out.len() - dist];
        out.push(byte);
    }
}

/// Compress each buffer at `level`, preserving order.
///
/// One buffer runs on the calling thread. More than one buffer is split across
/// `available_parallelism` workers. Every blob still decodes with [`decompress`].
pub fn compress_many(inputs: &[&[u8]], level: Level) -> Result<Vec<Vec<u8>>, &'static str> {
    let n = inputs.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    if n == 1 {
        let blob = if level == Level::Balanced {
            compress(inputs[0])?
        } else {
            compress_with(inputs[0], level)?
        };
        return Ok(vec![blob]);
    }
    let workers = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1)
        .clamp(1, n);
    let chunk = n.div_ceil(workers);
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for start in (0..n).step_by(chunk) {
            let end = (start + chunk).min(n);
            let part = &inputs[start..end];
            handles.push(scope.spawn(move || {
                part.iter()
                    .map(|bytes| compress_with(bytes, level))
                    .collect::<Result<Vec<_>, _>>()
            }));
        }
        let mut out = Vec::with_capacity(n);
        for handle in handles {
            out.extend(handle.join().expect("compress panicked")?);
        }
        Ok(out)
    })
}

fn hash3(data: &[u8], i: usize) -> usize {
    let mixed = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | data[i + 2] as u32;
    (mixed.wrapping_mul(0x1e35_a7bd) >> 16) as usize & 0xFFFF
}

fn insert(input: &[u8], i: usize, head: &mut [u32], prev: &mut [u32]) {
    if i + 3 > input.len() {
        return;
    }
    let hash = hash3(input, i);
    prev[i] = head[hash];
    head[hash] = i as u32 + 1;
}

fn find_match(
    input: &[u8],
    i: usize,
    head: &[u32],
    prev: &[u32],
    chain_limit: usize,
) -> (usize, usize) {
    if i + MIN_MATCH > input.len() {
        return (0, 0);
    }
    let hash = hash3(input, i);
    let mut cur = head[hash];
    let mut best_len = MIN_MATCH - 1;
    let mut best_dist = 0;
    let mut steps = 0;
    let max_len = (input.len() - i).min(MAX_MATCH);
    while cur != 0 && steps < chain_limit {
        steps += 1;
        let pos = (cur - 1) as usize;
        if pos >= i {
            break;
        }
        let dist = i - pos;
        if dist == 0 || dist > MAX_DISTANCE {
            break;
        }
        if input[pos..pos + MIN_MATCH] == input[i..i + MIN_MATCH] {
            let mut len = MIN_MATCH;
            while len < max_len && input[pos + len] == input[i + len] {
                len += 1;
            }
            if len > best_len {
                best_len = len;
                best_dist = dist;
                if best_len == max_len {
                    break;
                }
            }
        }
        cur = prev[pos];
    }
    if best_len >= MIN_MATCH {
        (best_len, best_dist)
    } else {
        (0, 0)
    }
}

fn emit_literals(out: &mut Vec<u8>, literals: &[u8]) {
    let mut rest = literals;
    while !rest.is_empty() {
        let n = rest.len().min(MAX_LITERAL);
        out.push((n - 1) as u8);
        out.extend_from_slice(&rest[..n]);
        rest = &rest[n..];
    }
}

fn emit_match(out: &mut Vec<u8>, len: usize, dist: usize) {
    debug_assert!((MIN_MATCH..=MAX_MATCH).contains(&len));
    debug_assert!((1..=MAX_DISTANCE).contains(&dist));
    out.push(0x80 | ((len - MIN_MATCH) as u8));
    out.extend_from_slice(&(dist as u16).to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(input: &[u8]) {
        let blob = compress(input).unwrap();
        let out = decompress(&blob).unwrap();
        assert_eq!(out, input);
    }

    #[test]
    fn empty_and_tiny() {
        roundtrip(b"");
        roundtrip(b"a");
        roundtrip(b"abc");
        roundtrip(&[b'x'; 3]);
    }

    #[test]
    fn literal_boundaries() {
        roundtrip(&[b'q'; 127]);
        roundtrip(&[b'q'; 128]);
        roundtrip(&[b'q'; 129]);
        roundtrip(&[b'q'; 256]);
    }

    #[test]
    fn levels_share_one_decoder() {
        let raw = b"abcabcabcabc-zzzzzzzzzzzzzzzz-abcabcabcabcabcabc";
        let mut sizes = Vec::new();
        for level in [Level::Fast, Level::Balanced, Level::Max] {
            let blob = compress_with(raw, level).unwrap();
            assert_eq!(decompress(&blob).unwrap(), raw);
            sizes.push(blob.len());
        }
        assert!(sizes[2] <= sizes[0]);
    }

    #[test]
    fn many_matches_single() {
        let raw: &[&[u8]] = &[b"abcabcabcabc", b"zzzzzzzzzzzzzzzz", b"hello"];
        let many = compress_many(raw, Level::Max).unwrap();
        for (input, blob) in raw.iter().zip(&many) {
            assert_eq!(decompress(blob).unwrap(), *input);
            assert_eq!(&compress_with(input, Level::Max).unwrap(), blob);
        }
    }

    #[test]
    fn repetitive_and_periodic() {
        roundtrip(&[b'a'; 10_000]);
        let mut data = Vec::new();
        for i in 0..5_000 {
            data.extend_from_slice(b"abcabc");
            data.push((i % 251) as u8);
        }
        roundtrip(&data);
    }

    #[test]
    fn pseudo_random() {
        let mut state = 0x1234_5678_9abc_def0u64;
        for &len in &[1usize, 2, 7, 64, 255, 1024, 4096, 20_000] {
            let mut data = vec![0u8; len];
            for byte in &mut data {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                *byte = (state >> 33) as u8;
            }
            roundtrip(&data);
        }
    }

    #[test]
    fn rejects_garbage() {
        assert!(decompress(b"").is_err());
        assert!(decompress(b"ULZ0\0\0\0\0").is_err());
        let mut huge = b"ULZ1".to_vec();
        huge.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(decompress(&huge).is_err());
        assert!(compress(&vec![0u8; MAX_UNCOMPRESSED + 1]).is_err());
    }

    #[test]
    fn corrupt_input_does_not_panic() {
        let blob = compress(b"hello hello hello hello hello").unwrap();
        for i in 0..blob.len() {
            let mut bad = blob.clone();
            bad[i] ^= 0xff;
            let _ = decompress(&bad);
        }
    }

    #[test]
    fn crafted_errors() {
        // literal that claims more bytes than the declared size
        let mut blob = b"ULZ1".to_vec();
        blob.extend_from_slice(&1u32.to_le_bytes());
        blob.push(1); // two literals
        blob.extend_from_slice(b"ab");
        assert!(decompress(&blob).is_err());

        // distance zero
        let mut blob = b"ULZ1".to_vec();
        blob.extend_from_slice(&4u32.to_le_bytes());
        blob.push(0x80);
        blob.extend_from_slice(&0u16.to_le_bytes());
        assert!(decompress(&blob).is_err());

        // trailing byte after a valid empty payload
        let mut blob = b"ULZ1".to_vec();
        blob.extend_from_slice(&0u32.to_le_bytes());
        blob.push(0);
        assert!(decompress(&blob).is_err());
    }
}
