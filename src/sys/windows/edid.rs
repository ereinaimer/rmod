//! Pure EDID decoding: physical size, gamma, chromaticity, CTA-861 HDR
//! metadata, and color-gamut coverage.
//!
//! Every function takes the raw EDID blob and returns `None` on invalid or
//! absent data — nothing here panics and nothing here touches the OS.

/// Physical panel size from the base block: bytes 21 (horizontal) and 22
/// (vertical) in centimeters. Both zero means "unknown", per the spec.
pub(crate) fn physical_size_cm(bytes: &[u8]) -> Option<(f32, f32)> {
    let (w, h) = (bytes.get(21).copied()?, bytes.get(22).copied()?);
    if w == 0 && h == 0 {
        None
    } else {
        Some((w as f32, h as f32))
    }
}

/// Gamma from base-block byte 23: `(byte + 100) / 100`. `0xFF` is
/// "undefined" per the spec and yields `None`.
pub(crate) fn gamma(bytes: &[u8]) -> Option<f32> {
    let raw = bytes.get(23).copied()?;
    if raw == 0xFF {
        None
    } else {
        Some((raw as f32 + 100.0) / 100.0)
    }
}

/// Chromaticity coordinates of the display's primaries and white point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Chromaticity {
    pub red: (f32, f32),
    pub green: (f32, f32),
    pub blue: (f32, f32),
    pub white: (f32, f32),
}

/// Decodes the 10-bit chromaticity coordinates from base-block bytes 25-34:
/// the 2 high bits of each coordinate live in bytes 25-26, the 8 low bits in
/// bytes 27-34. All-zero coordinates mean "unknown" and yield `None`.
pub(crate) fn chromaticity(bytes: &[u8]) -> Option<Chromaticity> {
    let high = (bytes.get(25).copied()?, bytes.get(26).copied()?);
    let low = bytes.get(27..35)?;
    let coord = |hi: u8, lo: u8| (((hi as u16) << 8) | lo as u16) as f32 / 1024.0;
    let c = Chromaticity {
        red: (
            coord(high.0 >> 6, low[0]),
            coord((high.0 >> 4) & 0x03, low[1]),
        ),
        green: (
            coord((high.0 >> 2) & 0x03, low[2]),
            coord(high.0 & 0x03, low[3]),
        ),
        blue: (
            coord(high.1 >> 6, low[4]),
            coord((high.1 >> 4) & 0x03, low[5]),
        ),
        white: (
            coord((high.1 >> 2) & 0x03, low[6]),
            coord(high.1 & 0x03, low[7]),
        ),
    };
    let all_zero = c.red == (0.0, 0.0)
        && c.green == (0.0, 0.0)
        && c.blue == (0.0, 0.0)
        && c.white == (0.0, 0.0);
    let in_range = [c.red, c.green, c.blue, c.white]
        .iter()
        .all(|(x, y)| (0.0..=1.0).contains(x) && (0.0..=1.0).contains(y));
    if all_zero || !in_range { None } else { Some(c) }
}

/// HDR static metadata advertised by a CTA-861 extension block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HdrEdid {
    /// HDR static metadata type bit 1: HDR10.
    pub hdr10: bool,
    /// HDR static metadata type bit 2: HLG.
    pub hlg: bool,
}

/// Reads HDR static metadata from the CTA-861 extension blocks (tag `0x02`)
/// of an EDID blob. Within a CTA block, data blocks start at offset 3; each
/// block starts with a byte whose high 3 bits are the tag and low 5 bits the
/// payload length. HDR static metadata is an extended data block: tag `0x07`
/// with extended tag `0x06` as its first payload byte, and the metadata-type
/// flags (SDR = bit 0, HDR10 = bit 1, HLG = bit 2) in the byte after that.
/// `None` when no CTA block with an HDR static metadata block exists.
pub(crate) fn hdr_metadata(bytes: &[u8]) -> Option<HdrEdid> {
    if bytes.len() < 128 {
        return None;
    }
    let count = bytes[126] as usize;
    for i in 0..count {
        let start = 128 + i * 128;
        if start + 128 > bytes.len() || bytes[start] != 0x02 {
            continue;
        }
        let block = &bytes[start..start + 128];
        let mut off = 3;
        while off + 1 < block.len() {
            let tag = block[off] >> 5;
            let len = (block[off] & 0x1F) as usize;
            if len == 0 || off + 1 + len > block.len() {
                break;
            }
            if tag == 0x07 && len >= 2 {
                let ext = block[off + 1];
                if ext == 0x06 {
                    let flags = block[off + 2];
                    return Some(HdrEdid {
                        hdr10: flags & 0x02 != 0,
                        hlg: flags & 0x04 != 0,
                    });
                }
            }
            off += 1 + len;
        }
    }
    None
}

/// Color-gamut coverage of the display against the sRGB and DCI-P3
/// reference triangles, as a percentage rounded to an integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GamutCoverage {
    pub srgb: u32,
    pub p3: u32,
}

/// Computes the share of the sRGB and DCI-P3 reference triangles covered by
/// the display's RGB triangle, as rounded percentages clamped to 100.
pub(crate) fn gamut_coverage(c: &Chromaticity) -> GamutCoverage {
    const SRGB: [(f32, f32); 3] = [(0.640, 0.330), (0.300, 0.600), (0.150, 0.060)];
    const P3: [(f32, f32); 3] = [(0.680, 0.320), (0.265, 0.690), (0.150, 0.060)];
    let monitor = [c.red, c.green, c.blue];

    fn coverage(subject: &[(f32, f32); 3], reference: &[(f32, f32); 3]) -> u32 {
        let ref_area = area(reference);
        if ref_area <= 0.0 {
            return 0;
        }
        let mut clipped = subject.to_vec();
        for i in 0..3 {
            clipped = clip(&clipped, reference[i], reference[(i + 1) % 3]);
        }
        let pct = area(&clipped) / ref_area * 100.0;
        pct.round().clamp(0.0, 100.0) as u32
    }

    GamutCoverage {
        srgb: coverage(&monitor, &SRGB),
        p3: coverage(&monitor, &P3),
    }
}

/// Sutherland–Hodgman: keeps the part of `poly` on the inside (left) of the
/// directed edge `a -> b`.
fn clip(poly: &[(f32, f32)], a: (f32, f32), b: (f32, f32)) -> Vec<(f32, f32)> {
    let d = (b.0 - a.0, b.1 - a.1);
    let mut out = Vec::with_capacity(poly.len() + 1);
    for i in 0..poly.len() {
        let cur = poly[i];
        let prev = poly[(i + poly.len() - 1) % poly.len()];
        let cur_in = cross(d, (cur.0 - a.0, cur.1 - a.1)) >= 0.0;
        let prev_in = cross(d, (prev.0 - a.0, prev.1 - a.1)) >= 0.0;
        if cur_in != prev_in {
            out.push(intersect(prev, cur, a, b));
        }
        if cur_in {
            out.push(cur);
        }
    }
    out
}

/// Intersection point of segments `p1-p2` and `p3-p4`.
fn intersect(p1: (f32, f32), p2: (f32, f32), p3: (f32, f32), p4: (f32, f32)) -> (f32, f32) {
    let d1 = (p2.0 - p1.0, p2.1 - p1.1);
    let d2 = (p4.0 - p3.0, p4.1 - p3.1);
    let denom = cross(d1, d2);
    if denom == 0.0 {
        return ((p1.0 + p2.0) / 2.0, (p1.1 + p2.1) / 2.0);
    }
    let t = cross((p3.0 - p1.0, p3.1 - p1.1), d2) / denom;
    (p1.0 + t * d1.0, p1.1 + t * d1.1)
}

/// 2D cross product of `a` and `b`.
fn cross(a: (f32, f32), b: (f32, f32)) -> f32 {
    a.0 * b.1 - a.1 * b.0
}

/// Polygon area via the shoelace formula; orientation does not matter.
fn area(points: &[(f32, f32)]) -> f32 {
    let mut sum = 0.0;
    for i in 0..points.len() {
        let (x1, y1) = points[i];
        let (x2, y2) = points[(i + 1) % points.len()];
        sum += x1 * y2 - x2 * y1;
    }
    (sum / 2.0).abs()
}

/// EDID data extracted from the monitor's cached EDID block.
pub(crate) struct EdidData {
    /// Display product name from the 0xFC descriptor.
    pub(crate) name: Option<String>,
    pub(crate) manufacturer: String,
    /// EDID product code (bytes 10-11, little-endian).
    pub(crate) product_code: u16,
    pub(crate) serial: String,
    /// First 8 hex chars of the SHA-256 of the raw EDID blob.
    pub(crate) fingerprint: String,
    pub(crate) manufactured_week: u8,
    pub(crate) manufactured_year: u16,
    pub(crate) native_width: u32,
    pub(crate) native_height: u32,
    pub(crate) native_refresh: u32,
    /// Physical panel size in centimeters, when the EDID reports it.
    #[allow(dead_code)]
    pub(crate) physical_size_cm: Option<(f32, f32)>,
    /// Gamma curve, when the EDID reports it.
    #[allow(dead_code)]
    pub(crate) gamma: Option<f32>,
    /// Display primaries and white point, when the EDID reports them.
    #[allow(dead_code)]
    pub(crate) chromaticity: Option<Chromaticity>,
    /// HDR static metadata (HDR10/HLG) from the CTA-861 extension.
    #[allow(dead_code)]
    pub(crate) hdr: Option<HdrEdid>,
}

/// Parses the base block of an EDID blob into monitor identity fields.
pub(crate) fn parse_edid(bytes: &[u8]) -> Result<EdidData, String> {
    if bytes.len() < 128 {
        return Err("EDID too short".to_string());
    }
    if bytes[..8] != [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00] {
        return Err("bad EDID header".to_string());
    }
    let manufacturer = pnp_manufacturer(&bytes[8..10]);
    let product_code = u16::from_le_bytes([bytes[10], bytes[11]]);
    let fingerprint = edid_fingerprint(bytes);
    let serial = String::from_utf8_lossy(&bytes[12..16])
        .trim_matches(|c: char| c == '\0' || c.is_whitespace())
        .to_string();
    let manufactured_week = bytes[16];
    let manufactured_year = if manufactured_week == 0 {
        let y = bytes[17];
        if y <= 0x0F {
            1990 + y as u16
        } else {
            2000 + y as u16
        }
    } else {
        1990 + bytes[17] as u16
    };
    let (native_width, native_height, native_refresh) = preferred_timing(&bytes[54..72]);

    let mut serial_descriptor = None;
    let mut product_name = None;
    for slot in [
        &bytes[54..72],
        &bytes[72..90],
        &bytes[90..108],
        &bytes[108..126],
    ] {
        if let Some((kind, text)) = display_descriptor(slot) {
            match kind {
                DescriptorKind::Serial => serial_descriptor = Some(text),
                DescriptorKind::ProductName => {
                    product_name.get_or_insert(text);
                }
            }
        }
    }
    let serial = if serial.is_empty() {
        serial_descriptor.unwrap_or_default()
    } else {
        serial
    };

    Ok(EdidData {
        name: product_name,
        manufacturer,
        product_code,
        serial,
        fingerprint,
        manufactured_week,
        manufactured_year,
        native_width,
        native_height,
        native_refresh,
        physical_size_cm: physical_size_cm(bytes),
        gamma: gamma(bytes),
        chromaticity: chromaticity(bytes),
        hdr: hdr_metadata(bytes),
    })
}

/// The kinds of display descriptors an EDID detailed-timing slot can hold.
enum DescriptorKind {
    Serial,
    ProductName,
}

/// Decodes a non-timing display descriptor slot (pixel clock zero) into its
/// kind and trimmed text, or `None` when the slot is a timing descriptor or
/// holds an unknown/reserved descriptor.
fn display_descriptor(slot: &[u8]) -> Option<(DescriptorKind, String)> {
    if slot.len() < 18 {
        return None;
    }
    let clock = ((slot[1] as u32) << 8) | slot[0] as u32;
    if clock != 0 {
        return None;
    }
    let kind = match slot[3] {
        0xFF => DescriptorKind::Serial,
        0xFC => DescriptorKind::ProductName,
        _ => return None,
    };
    let text = String::from_utf8_lossy(&slot[4..18])
        .trim_matches(|c: char| c == '\0' || c.is_control() || c == ' ')
        .to_string();
    if text.is_empty() {
        return None;
    }
    Some((kind, text))
}

/// Maps a PNP manufacturer code (the EDID manufacturer field) to a readable
/// brand name, or `None` when the code is unknown.
pub(crate) fn manufacturer_name(code: &str) -> Option<&'static str> {
    match code {
        "LEN" => Some("Lenovo"),
        "DEL" => Some("Dell"),
        "HPN" | "HWP" => Some("HP"),
        "SAM" | "SEC" => Some("Samsung"),
        "AOC" => Some("AOC"),
        "ACR" => Some("Acer"),
        "BNQ" => Some("BenQ"),
        "GSM" => Some("LG"),
        "VSC" => Some("ViewSonic"),
        "PHL" => Some("Philips"),
        "SHP" => Some("Sharp"),
        "TOS" => Some("Toshiba"),
        _ => None,
    }
}

/// Decodes the 3-letter PNP manufacturer code from EDID bytes 8-9
/// (e.g. bytes `0x30 0xAE` decode to "LEN").
fn pnp_manufacturer(pair: &[u8]) -> String {
    fn letter(v: u8) -> char {
        if (1..=26).contains(&v) {
            (b'A' + v - 1) as char
        } else {
            '?'
        }
    }
    let a = pair[0] >> 2;
    let b = ((pair[0] & 0x03) << 3) | (pair[1] >> 5);
    let c = pair[1] & 0x1F;
    format!("{}{}{}", letter(a), letter(b), letter(c))
}

/// Reads the preferred timing (first detailed timing descriptor) and
/// returns `(width, height, refresh)`, or `(0, 0, 0)` when the descriptor
/// is a non-timing descriptor or the values are implausible.
fn preferred_timing(dtd: &[u8]) -> (u32, u32, u32) {
    let clock = ((dtd[1] as u32) << 8) | dtd[0] as u32;
    if clock == 0 {
        return (0, 0, 0);
    }
    let width = (((dtd[4] & 0x0F) as u32) << 8) | dtd[2] as u32;
    let height = (((dtd[7] & 0x0F) as u32) << 8) | dtd[5] as u32;
    let h_blank = (((dtd[4] >> 4) as u32) << 8) | dtd[3] as u32;
    let v_blank = (((dtd[7] >> 4) as u32) << 8) | dtd[6] as u32;
    let h_total = width + h_blank;
    let v_total = height + v_blank;
    if h_total == 0 || v_total == 0 {
        return (0, 0, 0);
    }
    let refresh = clock * 10_000 / (h_total * v_total);
    if (320..=7680).contains(&width)
        && (200..=4320).contains(&height)
        && (24..=300).contains(&refresh)
    {
        (width, height, refresh)
    } else {
        (0, 0, 0)
    }
}

/// Builds the base display name from EDID identity, following the industry
/// convention (ddcutil, edid-decode, fastfetch): the 0xFC product name when
/// present, otherwise the manufacturer brand plus the hex product code
/// (e.g. "Lenovo 9059"), otherwise the Windows friendly name.
pub(crate) fn base_display_name(
    product_name: Option<String>,
    manufacturer: &str,
    product_code: u16,
    friendly: String,
) -> String {
    if let Some(name) = product_name {
        return name;
    }
    if let Some(brand) = manufacturer_name(manufacturer) {
        return format!("{brand} {product_code:04X}");
    }
    friendly
}

/// Appends the EDID fingerprint to a base display name: `Name [a1b2c3d4]`.
/// The suffix is omitted when no fingerprint is available (EDID read failed).
pub(crate) fn append_fingerprint(name: String, fingerprint: &str) -> String {
    if fingerprint.is_empty() {
        name
    } else {
        format!("{name} [{fingerprint}]")
    }
}

/// The first 8 hex characters of the SHA-256 of the raw EDID blob: a stable,
/// per-panel identifier that works even when the panel ships no serial.
fn edid_fingerprint(bytes: &[u8]) -> String {
    sha256(bytes)
        .iter()
        .take(4)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// SHA-256 of `data` as a `[u8; 32]`. Implemented inline (the crate has no
/// dependencies) following FIPS 180-4.
fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a_2f98,
        0x7137_4491,
        0xb5c0_fbcf,
        0xe9b5_dba5,
        0x3956_c25b,
        0x59f1_11f1,
        0x923f_82a4,
        0xab1c_5ed5,
        0xd807_aa98,
        0x1283_5b01,
        0x2431_85be,
        0x550c_7dc3,
        0x72be_5d74,
        0x80de_b1fe,
        0x9bdc_06a7,
        0xc19b_f174,
        0xe49b_69c1,
        0xefbe_4786,
        0x0fc1_9dc6,
        0x240c_a1cc,
        0x2de9_2c6f,
        0x4a74_84aa,
        0x5cb0_a9dc,
        0x76f9_88da,
        0x983e_5152,
        0xa831_c66d,
        0xb003_27c8,
        0xbf59_7fc7,
        0xc6e0_0bf3,
        0xd5a7_9147,
        0x06ca_6351,
        0x1429_2967,
        0x27b7_0a85,
        0x2e1b_2138,
        0x4d2c_6dfc,
        0x5338_0d13,
        0x650a_7354,
        0x766a_0abb,
        0x81c2_c92e,
        0x9272_2c85,
        0xa2bf_e8a1,
        0xa81a_664b,
        0xc24b_8b70,
        0xc76c_51a3,
        0xd192_e819,
        0xd699_0624,
        0xf40e_3585,
        0x106a_a070,
        0x19a4_c116,
        0x1e37_6c08,
        0x2748_774c,
        0x34b0_bcb5,
        0x391c_0cb3,
        0x4ed8_aa4a,
        0x5b9c_ca4f,
        0x682e_6ff3,
        0x748f_82ee,
        0x78a5_636f,
        0x84c8_7814,
        0x8cc7_0208,
        0x90be_fffa,
        0xa450_6ceb,
        0xbef9_a3f7,
        0xc671_78f2,
    ];
    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    let mut h: [u32; 8] = [
        0x6a09_e667,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_edid() -> Vec<u8> {
        let mut b = vec![0u8; 128];
        b[..8].copy_from_slice(&[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]);
        b
    }

    fn cta_edid(payload_at_131: &[(usize, u8)]) -> Vec<u8> {
        let mut b = base_edid();
        b[126] = 1;
        b.extend_from_slice(&[0u8; 128]);
        b[128] = 0x02; // CTA-861 extension tag
        for (offset, value) in payload_at_131 {
            b[131 + offset] = *value;
        }
        b
    }

    /// Writes a non-timing display descriptor into a 18-byte EDID slot. The
    /// descriptor text field is 13 bytes; longer text is truncated.
    fn put_descriptor(b: &mut [u8], offset: usize, tag: u8, text: &str) {
        b[offset..offset + 3].fill(0);
        b[offset + 3] = tag;
        b[offset + 4..offset + 18].fill(0);
        let bytes = text.as_bytes();
        let n = bytes.len().min(13);
        b[offset + 4..offset + 4 + n].copy_from_slice(&bytes[..n]);
    }

    #[test]
    fn physical_size_cm_reads_base_block_bytes() {
        let mut b = base_edid();
        b[21] = 48; // 48 cm wide
        b[22] = 27; // 27 cm tall
        assert_eq!(physical_size_cm(&b), Some((48.0, 27.0)));
    }

    #[test]
    fn physical_size_cm_zeros_are_none() {
        assert_eq!(physical_size_cm(&base_edid()), None);
    }

    #[test]
    fn physical_size_cm_short_blob_is_none() {
        assert_eq!(physical_size_cm(&[0u8; 20]), None);
    }

    #[test]
    fn gamma_computes_from_byte_23() {
        let mut b = base_edid();
        b[23] = 120; // (120 + 100) / 100 = 2.2
        assert_eq!(gamma(&b), Some(2.2));
    }

    #[test]
    fn gamma_ff_is_none() {
        let mut b = base_edid();
        b[23] = 0xFF; // undefined per spec
        assert_eq!(gamma(&b), None);
    }

    #[test]
    fn gamma_short_blob_is_none() {
        assert_eq!(gamma(&[0u8; 10]), None);
    }

    #[test]
    fn chromaticity_decodes_known_values() {
        let mut b = base_edid();
        // sRGB primaries, D65 white, encoded as 10-bit values:
        // red(0.64, 0.33), green(0.30, 0.60), blue(0.15, 0.06),
        // white(0.3127, 0.3290)
        b[25] = 0x96; // rx hi 2, ry hi 1, gx hi 1, gy hi 2
        b[26] = 0x05; // bx hi 0, by hi 0, wx hi 1, wy hi 1
        b[27..35].copy_from_slice(&[0x8F, 0x52, 0x33, 0x66, 0x9A, 0x3D, 0x40, 0x51]);
        let c = chromaticity(&b).unwrap();
        assert!((c.red.0 - 0.64).abs() < 0.001, "red x = {}", c.red.0);
        assert!((c.red.1 - 0.33).abs() < 0.001, "red y = {}", c.red.1);
        assert!((c.green.0 - 0.30).abs() < 0.001, "green x = {}", c.green.0);
        assert!((c.green.1 - 0.60).abs() < 0.001, "green y = {}", c.green.1);
        assert!((c.blue.0 - 0.15).abs() < 0.001, "blue x = {}", c.blue.0);
        assert!((c.blue.1 - 0.06).abs() < 0.001, "blue y = {}", c.blue.1);
        assert!(
            (c.white.0 - 0.3127).abs() < 0.001,
            "white x = {}",
            c.white.0
        );
        assert!(
            (c.white.1 - 0.3290).abs() < 0.001,
            "white y = {}",
            c.white.1
        );
    }

    #[test]
    fn chromaticity_all_zero_is_none() {
        assert_eq!(chromaticity(&base_edid()), None);
    }

    #[test]
    fn chromaticity_short_blob_is_none() {
        assert_eq!(chromaticity(&[0u8; 24]), None);
    }

    #[test]
    fn hdr_metadata_reads_cta_extension_hdr10() {
        // HDR static metadata is an extended data block (tag 0x07): the
        // byte at 131 carries the tag and length, 132 the extended tag
        // 0x06, and 133 the metadata-type flags (bit 1 = HDR10).
        let b = cta_edid(&[(0, (0x07 << 5) | 4), (1, 0x06), (2, 0x02)]);
        let hdr = hdr_metadata(&b).unwrap();
        assert!(hdr.hdr10);
        assert!(!hdr.hlg);
    }

    #[test]
    fn hdr_metadata_reads_hlg_bit() {
        let b = cta_edid(&[(0, (0x07 << 5) | 4), (1, 0x06), (2, 0x04)]);
        let hdr = hdr_metadata(&b).unwrap();
        assert!(hdr.hlg);
        assert!(!hdr.hdr10);
    }

    #[test]
    fn hdr_metadata_reads_both_bits() {
        let b = cta_edid(&[(0, (0x07 << 5) | 4), (1, 0x06), (2, 0x06)]);
        let hdr = hdr_metadata(&b).unwrap();
        assert!(hdr.hdr10);
        assert!(hdr.hlg);
    }

    #[test]
    fn hdr_metadata_skips_other_data_blocks() {
        // video data block (tag 0x02, len 2) precedes the HDR extended
        // data block
        let b = cta_edid(&[
            (0, (0x02 << 5) | 2),
            (1, 0x01),
            (2, 0x02),
            (3, (0x07 << 5) | 4),
            (4, 0x06),
            (5, 0x02),
        ]);
        let hdr = hdr_metadata(&b).unwrap();
        assert!(hdr.hdr10);
        assert!(!hdr.hlg);
    }

    #[test]
    fn hdr_metadata_skips_bare_hdr_looking_block() {
        // a bare tag-0x03 block (the old, wrong layout) must be skipped;
        // the real HDR extended data block after it is what counts
        let b = cta_edid(&[
            (0, (0x03 << 5) | 5),
            (1, 0x02),
            (2, 0x00),
            (3, 0x00),
            (4, 0x00),
            (5, 0x00),
            (6, (0x07 << 5) | 4),
            (7, 0x06),
            (8, 0x04),
        ]);
        let hdr = hdr_metadata(&b).unwrap();
        assert!(hdr.hlg);
        assert!(!hdr.hdr10);
    }

    #[test]
    fn hdr_metadata_skips_non_hdr_extended_block() {
        // an extended data block with a non-HDR extended tag (0x04 =
        // dynamic metadata) is skipped, then the HDR block is found
        let b = cta_edid(&[
            (0, (0x07 << 5) | 4),
            (1, 0x04),
            (2, 0x00),
            (5, (0x07 << 5) | 4),
            (6, 0x06),
            (7, 0x02),
        ]);
        let hdr = hdr_metadata(&b).unwrap();
        assert!(hdr.hdr10);
        assert!(!hdr.hlg);
    }

    #[test]
    fn hdr_metadata_none_for_non_hdr_extended_block_only() {
        let b = cta_edid(&[(0, (0x07 << 5) | 4), (1, 0x00), (2, 0x00)]);
        assert_eq!(hdr_metadata(&b), None);
    }

    #[test]
    fn hdr_metadata_none_without_extension() {
        assert_eq!(hdr_metadata(&base_edid()), None);
    }

    #[test]
    fn hdr_metadata_none_for_non_cta_extension() {
        let mut b = base_edid();
        b[126] = 1;
        b.extend_from_slice(&[0u8; 128]);
        b[128] = 0x60; // DisplayID extension, not CTA-861
        assert_eq!(hdr_metadata(&b), None);
    }

    #[test]
    fn hdr_metadata_none_when_no_hdr_block() {
        let b = cta_edid(&[(0, (0x02 << 5) | 2), (1, 0x01), (2, 0x02)]);
        assert_eq!(hdr_metadata(&b), None);
    }

    #[test]
    fn hdr_metadata_short_blob_is_none() {
        assert_eq!(hdr_metadata(&[0u8; 127]), None);
    }

    #[test]
    fn hdr_metadata_finds_cta_among_multiple_extensions() {
        let mut b = base_edid();
        b[126] = 2;
        b.extend_from_slice(&[0u8; 256]);
        b[128] = 0x60; // non-CTA first extension
        b[256] = 0x02; // CTA second extension
        b[259] = (0x07 << 5) | 4;
        b[260] = 0x06;
        b[261] = 0x02;
        let hdr = hdr_metadata(&b).unwrap();
        assert!(hdr.hdr10);
    }

    #[test]
    fn area_uses_shoelace_absolute() {
        let tri = [(0.640, 0.330), (0.300, 0.600), (0.150, 0.060)];
        let a = area(&tri);
        assert!((a - 0.11205).abs() < 0.0001, "area = {a}");
        let reversed = [(0.150, 0.060), (0.300, 0.600), (0.640, 0.330)];
        assert!(
            (area(&reversed) - a).abs() < 0.0001,
            "orientation must not matter"
        );
    }

    #[test]
    fn gamut_coverage_srgb_triangle_is_100_percent_srgb() {
        let c = Chromaticity {
            red: (0.640, 0.330),
            green: (0.300, 0.600),
            blue: (0.150, 0.060),
            white: (0.3127, 0.3290),
        };
        let cov = gamut_coverage(&c);
        assert_eq!(cov.srgb, 100);
        // The sRGB triangle lies entirely inside P3, so the P3 coverage is
        // area(sRGB) / area(P3) * 100 rounded, with tolerance for f32 math.
        assert!((cov.p3 as i32 - 74).abs() <= 1, "p3 = {}", cov.p3);
    }

    #[test]
    fn gamut_coverage_tiny_triangle_is_low() {
        let c = Chromaticity {
            red: (0.350, 0.350),
            green: (0.360, 0.350),
            blue: (0.350, 0.360),
            white: (0.3127, 0.3290),
        };
        let cov = gamut_coverage(&c);
        assert!(cov.srgb <= 1, "srgb = {}", cov.srgb);
        assert!(cov.p3 <= 1, "p3 = {}", cov.p3);
    }

    #[test]
    fn gamut_coverage_no_overlap_is_zero() {
        let c = Chromaticity {
            red: (0.900, 0.300),
            green: (0.950, 0.350),
            blue: (0.850, 0.350),
            white: (0.3127, 0.3290),
        };
        let cov = gamut_coverage(&c);
        assert_eq!(cov.srgb, 0);
        assert_eq!(cov.p3, 0);
    }

    #[test]
    fn parse_edid_reads_manufacturer_serial_and_dates() {
        let mut b = base_edid();
        b[8] = 0x30;
        b[9] = 0xAE; // LEN
        b[12..16].copy_from_slice(b"ABC1");
        b[16] = 34;
        b[17] = 29; // week 34, year 2019
        let edid = parse_edid(&b).unwrap();
        assert_eq!(edid.manufacturer, "LEN");
        assert_eq!(edid.serial, "ABC1");
        assert_eq!(edid.manufactured_week, 34);
        assert_eq!(edid.manufactured_year, 2019);
    }

    #[test]
    fn parse_edid_trims_trailing_nuls_in_serial() {
        let mut b = base_edid();
        b[12..16].copy_from_slice(&[b'A', 0, 0, 0]);
        assert_eq!(parse_edid(&b).unwrap().serial, "A");
    }

    #[test]
    fn parse_edid_model_year_when_week_is_zero() {
        let mut b = base_edid();
        b[16] = 0;
        b[17] = 0x05;
        assert_eq!(parse_edid(&b).unwrap().manufactured_year, 1995);

        let mut b = base_edid();
        b[16] = 0;
        b[17] = 0x18;
        assert_eq!(parse_edid(&b).unwrap().manufactured_year, 2024);
    }

    #[test]
    fn parse_edid_rejects_short_blob_and_bad_header() {
        assert!(parse_edid(&[0u8; 16]).is_err());
        let mut b = base_edid();
        b[0] = 0x01;
        assert!(parse_edid(&b).is_err());
    }

    #[test]
    fn pnp_manufacturer_decodes_len() {
        assert_eq!(pnp_manufacturer(&[0x30, 0xAE]), "LEN");
        assert_eq!(pnp_manufacturer(&[0x10, 0xAC]), "DEL");
        assert_eq!(pnp_manufacturer(&[0x4C, 0x2D]), "SAM");
    }

    #[test]
    fn parse_edid_reads_product_code_little_endian() {
        let mut b = base_edid();
        b[10] = 0x59;
        b[11] = 0x90;
        assert_eq!(parse_edid(&b).unwrap().product_code, 0x9059);
    }

    #[test]
    fn parse_edid_computes_stable_fingerprint() {
        let b = base_edid();
        let fp = parse_edid(&b).unwrap().fingerprint;
        assert_eq!(fp.len(), 8, "fingerprint is 8 hex chars");
        assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(parse_edid(&b).unwrap().fingerprint, fp, "deterministic");
    }

    #[test]
    fn edid_fingerprint_changes_with_blob() {
        let a = base_edid();
        let mut b = base_edid();
        b[16] = 34;
        assert_ne!(edid_fingerprint(&a), edid_fingerprint(&b));
    }

    #[test]
    fn sha256_matches_known_vector() {
        // SHA-256("abc")
        assert_eq!(
            sha256(b"abc"),
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
                0xf2, 0x00, 0x15, 0xad,
            ]
        );
    }

    #[test]
    fn base_display_name_prefers_product_name() {
        assert_eq!(
            base_display_name(
                Some("DELL P2411H".to_string()),
                "DEL",
                0x5e42,
                "Generic PnP Monitor".to_string()
            ),
            "DELL P2411H"
        );
    }

    #[test]
    fn base_display_name_uses_brand_and_product_code() {
        assert_eq!(
            base_display_name(None, "LEN", 0x9059, "Generic PnP Monitor".to_string()),
            "Lenovo 9059"
        );
    }

    #[test]
    fn base_display_name_unknown_manufacturer_uses_friendly() {
        assert_eq!(
            base_display_name(None, "XYZ", 0x1234, "Generic PnP Monitor".to_string()),
            "Generic PnP Monitor"
        );
    }

    #[test]
    fn append_fingerprint_adds_suffix() {
        assert_eq!(
            append_fingerprint("Lenovo 9059".to_string(), "a1b2c3d4"),
            "Lenovo 9059 [a1b2c3d4]"
        );
    }

    #[test]
    fn append_fingerprint_omits_empty_suffix() {
        assert_eq!(append_fingerprint("X".to_string(), ""), "X");
    }

    #[test]
    fn preferred_timing_decodes_1920x1080_at_60hz() {
        let mut dtd = [0u8; 18];
        // pixel clock 148.5 MHz, h 1920(+280 blank), v 1080(+45 blank)
        dtd[0] = 0x02;
        dtd[1] = 0x3A; // 0x3A02 = 14850 units of 10 kHz
        dtd[2] = 0x80;
        dtd[3] = 0x18;
        dtd[4] = 0x17; // h blank hi 1, h active hi 7
        dtd[5] = 0x38;
        dtd[6] = 0x2D;
        dtd[7] = 0x04; // v blank hi 0, v active hi 4
        let (w, h, r) = preferred_timing(&dtd);
        assert_eq!((w, h, r), (1920, 1080, 60));
    }

    #[test]
    fn preferred_timing_ignores_zero_clock_descriptor() {
        let dtd = [0u8; 18];
        assert_eq!(preferred_timing(&dtd), (0, 0, 0));
    }

    #[test]
    fn preferred_timing_rejects_implausible_resolution() {
        let mut dtd = [0u8; 18];
        dtd[0] = 0x6F;
        dtd[1] = 0x54; // 216.15 MHz
        dtd[2] = 0x80;
        dtd[3] = 0x9C;
        dtd[4] = 0x70; // h active hi 0 -> width 128
        dtd[5] = 0x38;
        dtd[6] = 0x3E;
        dtd[7] = 0x40;
        assert_eq!(preferred_timing(&dtd), (0, 0, 0));
    }

    #[test]
    fn parse_edid_reads_product_name_descriptor() {
        let mut b = base_edid();
        put_descriptor(&mut b, 72, 0xFC, "XYZZY 9000");
        assert_eq!(parse_edid(&b).unwrap().name.as_deref(), Some("XYZZY 9000"));
    }

    #[test]
    fn parse_edid_ignores_ascii_string_descriptor_for_name() {
        let mut b = base_edid();
        put_descriptor(&mut b, 108, 0xFE, "B156HAN13.1");
        assert_eq!(parse_edid(&b).unwrap().name, None);
    }

    #[test]
    fn parse_edid_reads_serial_descriptor_when_serial_field_empty() {
        let mut b = base_edid();
        put_descriptor(&mut b, 90, 0xFF, "SN1234567");
        assert_eq!(parse_edid(&b).unwrap().serial, "SN1234567");
    }

    #[test]
    fn parse_edid_prefers_serial_field_over_descriptor() {
        let mut b = base_edid();
        b[12..16].copy_from_slice(b"ABC1");
        put_descriptor(&mut b, 90, 0xFF, "SN1234567");
        assert_eq!(parse_edid(&b).unwrap().serial, "ABC1");
    }

    #[test]
    fn parse_edid_prefers_product_name_over_ascii_string() {
        let mut b = base_edid();
        put_descriptor(&mut b, 72, 0xFE, "GenericStr");
        put_descriptor(&mut b, 90, 0xFC, "RealModel");
        assert_eq!(parse_edid(&b).unwrap().name.as_deref(), Some("RealModel"));
    }

    #[test]
    fn manufacturer_name_maps_common_brands() {
        assert_eq!(manufacturer_name("LEN"), Some("Lenovo"));
        assert_eq!(manufacturer_name("DEL"), Some("Dell"));
        assert_eq!(manufacturer_name("HPN"), Some("HP"));
        assert_eq!(manufacturer_name("SAM"), Some("Samsung"));
        assert_eq!(manufacturer_name("XYZ"), None);
    }

    #[test]
    fn parse_edid_ignores_unknown_and_range_limit_descriptors() {
        let mut b = base_edid();
        put_descriptor(&mut b, 72, 0x0F, "dummy"); // reserved tag
        put_descriptor(&mut b, 90, 0xFD, ""); // range limits, no text
        let edid = parse_edid(&b).unwrap();
        assert_eq!(edid.name, None);
        assert_eq!(edid.serial, "");
    }

    #[test]
    fn parse_edid_matches_lenovo_b156han_real_blob() {
        // The base block captured from this machine's Lenovo panel.
        let b = [
            0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x30, 0xAE, 0x59, 0x90, 0x00, 0x00,
            0x00, 0x00, 0x22, 0x1D, 0x01, 0x04, 0xA5, 0x22, 0x13, 0x78, 0x03, 0x48, 0x35, 0x8F,
            0x57, 0x59, 0x92, 0x29, 0x1E, 0x50, 0x54, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x01,
            0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x54, 0x6F,
            0x80, 0x9C, 0x70, 0x38, 0x3E, 0x40, 0x6C, 0x30, 0xAA, 0x00, 0x58, 0xC1, 0x10, 0x00,
            0x00, 0x18, 0x00, 0x00, 0x00, 0x0F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0xFD, 0x00, 0x3C, 0x78, 0x8A,
            0x8A, 0x1D, 0x01, 0x0A, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x00, 0x00, 0x00, 0xFE,
            0x00, 0x42, 0x31, 0x35, 0x36, 0x48, 0x41, 0x4E, 0x31, 0x33, 0x2E, 0x31, 0x20, 0x0A,
            0x00, 0x55,
        ];
        assert_eq!(b.len(), 128);
        let edid = parse_edid(&b).unwrap();
        assert_eq!(edid.manufacturer, "LEN");
        assert_eq!(edid.product_code, 0x9059);
        assert_eq!(edid.name, None); // panel model lives in a 0xFE string, not a 0xFC product name
        assert_eq!(edid.serial, ""); // this panel ships with no EDID serial
        assert_eq!(edid.fingerprint.len(), 8);
        assert!(
            edid.fingerprint.chars().all(|c| c.is_ascii_hexdigit()),
            "fingerprint must be hex, got '{}'",
            edid.fingerprint
        );
        assert_eq!(edid.manufactured_week, 34);
        assert_eq!(edid.manufactured_year, 2019);
        assert_eq!((edid.native_width, edid.native_height), (0, 0)); // DTD1 implausible
    }

    #[test]
    fn parse_edid_reads_size_gamma_chromaticity_and_hdr() {
        let mut b = base_edid();
        b[21] = 48;
        b[22] = 27;
        b[23] = 120;
        // sRGB primaries, D65 white, 10-bit encoded
        b[25] = 0x96;
        b[26] = 0x05;
        b[27..35].copy_from_slice(&[0x8F, 0x52, 0x33, 0x66, 0x9A, 0x3D, 0x40, 0x51]);
        // CTA-861 extension with HDR10 static metadata (extended data block)
        b[126] = 1;
        b.extend_from_slice(&[0u8; 128]);
        b[128] = 0x02;
        b[131] = (0x07 << 5) | 4;
        b[132] = 0x06;
        b[133] = 0x02;
        let edid = parse_edid(&b).unwrap();
        assert_eq!(edid.physical_size_cm, Some((48.0, 27.0)));
        assert_eq!(edid.gamma, Some(2.2));
        let c = edid.chromaticity.unwrap();
        assert!((c.red.0 - 0.64).abs() < 0.001);
        let hdr = edid.hdr.unwrap();
        assert!(hdr.hdr10);
        assert!(!hdr.hlg);
    }

    #[test]
    fn parse_edid_defaults_absent_edid_fields_to_none() {
        let mut b = base_edid();
        b[23] = 0xFF; // gamma "undefined" per spec
        let edid = parse_edid(&b).unwrap();
        assert_eq!(edid.physical_size_cm, None);
        assert_eq!(edid.gamma, None);
        assert_eq!(edid.chromaticity, None);
        assert_eq!(edid.hdr, None);
    }
}
