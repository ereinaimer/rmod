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
        red: (coord(high.0 >> 6, low[0]), coord((high.0 >> 4) & 0x03, low[1])),
        green: (coord((high.0 >> 2) & 0x03, low[2]), coord(high.0 & 0x03, low[3])),
        blue: (coord(high.1 >> 6, low[4]), coord((high.1 >> 4) & 0x03, low[5])),
        white: (coord((high.1 >> 2) & 0x03, low[6]), coord(high.1 & 0x03, low[7])),
    };
    let all_zero = c.red == (0.0, 0.0)
        && c.green == (0.0, 0.0)
        && c.blue == (0.0, 0.0)
        && c.white == (0.0, 0.0);
    let in_range = [c.red, c.green, c.blue, c.white]
        .iter()
        .all(|(x, y)| (0.0..=1.0).contains(x) && (0.0..=1.0).contains(y));
    if all_zero || !in_range {
        None
    } else {
        Some(c)
    }
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
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GamutCoverage {
    pub srgb: u32,
    pub p3: u32,
}

/// Computes the share of the sRGB and DCI-P3 reference triangles covered by
/// the display's RGB triangle, as rounded percentages clamped to 100.
#[allow(dead_code)]
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
        assert!((c.white.0 - 0.3127).abs() < 0.001, "white x = {}", c.white.0);
        assert!((c.white.1 - 0.3290).abs() < 0.001, "white y = {}", c.white.1);
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
        assert!((area(&reversed) - a).abs() < 0.0001, "orientation must not matter");
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
}