//! The thumbhash of an image: its colours and shapes in about 25 bytes, drawn back as a tiny
//! blurred image.
//!
//! Ported from Evan Wallace's reference implementation (<https://github.com/evanw/thumbhash>,
//! the `thumbhash` crate 0.1.0, MIT, Copyright (c) 2023 Evan Wallace). The arithmetic and its
//! order are the original's; the cosines go through `libm`, so the bytes are the same on every
//! machine, and indexing became iteration.

#![expect(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::cast_precision_loss,
    clippy::manual_midpoint,
    clippy::too_many_lines,
    reason = "the original's names, arithmetic and functions, kept so the port reads against it"
)]

use std::f32::consts::PI;

/// The most pixels a side may have: a larger image takes longer for nothing.
pub(crate) const SIDE: u32 = 100;

/// How many pixels the longer side of a drawn hash has.
const DRAWN: f32 = 32.0;

/// A thumbhash: about 25 bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Thumbhash(Vec<u8>);

/// RGBA pixels, row by row, not premultiplied.
#[derive(Debug)]
pub(crate) struct Rgba<'a> {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) pixels: &'a [u8],
}

/// A channel's constant term, its varying terms (each with its horizontal and vertical
/// frequency) and their scale.
#[derive(Debug)]
struct Channel {
    dc: f32,
    ac: Vec<(usize, usize, f32)>,
    scale: f32,
}

impl Rgba<'_> {
    /// The cosine of frequency `c` at pixel `at` of `n`.
    fn cos(n: usize, c: usize, at: usize) -> f32 {
        libm::cosf(PI / n as f32 * c as f32 * (at as f32 + 0.5))
    }

    /// `values` (one per pixel) through the DCT up to `nx` by `ny` frequencies.
    fn channel(&self, values: &[f32], nx: usize, ny: usize) -> Channel {
        let (w, h) = (self.width, self.height);
        let mut channel = Channel { dc: 0.0, ac: Vec::new(), scale: 0.0 };
        for cy in 0..ny {
            let mut cx = 0;
            while cx * ny < nx * (ny - cy) {
                let fx: Vec<f32> = (0..w).map(|x| Self::cos(w, cx, x)).collect();
                let mut f = 0.0;
                for (y, row) in values.chunks_exact(w.max(1)).take(h).enumerate() {
                    let fy = Self::cos(h, cy, y);
                    for (value, fx) in row.iter().zip(&fx) {
                        f += value * fx * fy;
                    }
                }
                f /= (w * h) as f32;
                if cx > 0 || cy > 0 {
                    channel.ac.push((cx, cy, f));
                    channel.scale = f.abs().max(channel.scale);
                } else {
                    channel.dc = f;
                }
                cx += 1;
            }
        }
        if channel.scale > 0.0 {
            for (_, _, ac) in &mut channel.ac {
                *ac = 0.5 + 0.5 / channel.scale * *ac;
            }
        }
        channel
    }
}

impl Thumbhash {
    /// The hash of `image`, at most [`SIDE`] pixels on each side.
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "the original's quantization")]
    pub(crate) fn of(image: &Rgba<'_>) -> Self {
        let (w, h) = (image.width, image.height);
        let (mut avg_r, mut avg_g, mut avg_b, mut avg_a) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        let pixels = || image.pixels.as_chunks::<4>().0.iter().copied();
        for [r, g, b, a] in pixels() {
            let alpha = f32::from(a) / 255.0;
            avg_r += alpha / 255.0 * f32::from(r);
            avg_g += alpha / 255.0 * f32::from(g);
            avg_b += alpha / 255.0 * f32::from(b);
            avg_a += alpha;
        }
        if avg_a > 0.0 {
            avg_r /= avg_a;
            avg_g /= avg_a;
            avg_b /= avg_a;
        }
        let has_alpha = avg_a < (w * h) as f32;
        let l_limit = if has_alpha { 5 } else { 7 };
        let longer = w.max(h) as f32;
        let lx = ((((l_limit * w) as f32) / longer).round() as usize).max(1);
        let ly = ((((l_limit * h) as f32) / longer).round() as usize).max(1);
        let (mut l, mut p, mut q, mut a) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for [pr, pg, pb, pa] in pixels() {
            let alpha = f32::from(pa) / 255.0;
            let r = avg_r * (1.0 - alpha) + alpha / 255.0 * f32::from(pr);
            let g = avg_g * (1.0 - alpha) + alpha / 255.0 * f32::from(pg);
            let b = avg_b * (1.0 - alpha) + alpha / 255.0 * f32::from(pb);
            l.push((r + g + b) / 3.0);
            p.push((r + g) / 2.0 - b);
            q.push(r - g);
            a.push(alpha);
        }
        let l = image.channel(&l, lx.max(3), ly.max(3));
        let p = image.channel(&p, 3, 3);
        let q = image.channel(&q, 3, 3);
        let a = has_alpha.then(|| image.channel(&a, 5, 5));
        let is_landscape = w > h;
        let header24 = (63.0 * l.dc).round() as u32
            | (((31.5 + 31.5 * p.dc).round() as u32) << 6)
            | (((31.5 + 31.5 * q.dc).round() as u32) << 12)
            | (((31.0 * l.scale).round() as u32) << 18)
            | if has_alpha { 1 << 23 } else { 0 };
        let header16 = (if is_landscape { ly } else { lx }) as u16
            | (((63.0 * p.scale).round() as u16) << 3)
            | (((63.0 * q.scale).round() as u16) << 9)
            | if is_landscape { 1 << 15 } else { 0 };
        let mut hash = Vec::with_capacity(25);
        hash.extend_from_slice(&header24.to_le_bytes()[..3]);
        hash.extend_from_slice(&header16.to_le_bytes());
        if let Some(a) = &a {
            hash.push((15.0 * a.dc).round() as u8 | (((15.0 * a.scale).round() as u8) << 4));
        }
        let nibbles: Vec<u8> = [Some(&l), Some(&p), Some(&q), a.as_ref()]
            .into_iter()
            .flatten()
            .flat_map(|channel| channel.ac.iter().map(|(_, _, f)| (15.0 * f).round() as u8))
            .collect();
        for pair in nibbles.chunks(2) {
            hash.push(pair.iter().rev().fold(0, |byte, nibble| (byte << 4) | nibble));
        }
        Self(hash)
    }

    /// The hash drawn as RGBA pixels: width, height and the pixels, the longer side 32 pixels;
    /// `None` when the hash is too short.
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "the original's quantization")]
    pub(crate) fn draw(&self) -> Option<(usize, usize, Vec<u8>)> {
        let hash = self.0.as_slice();
        let header24 = u32::from_le_bytes([*hash.first()?, *hash.get(1)?, *hash.get(2)?, 0]);
        let header16 = u16::from_le_bytes([*hash.get(3)?, *hash.get(4)?]);
        let l_dc = (header24 & 63) as f32 / 63.0;
        let p_dc = ((header24 >> 6) & 63) as f32 / 31.5 - 1.0;
        let q_dc = ((header24 >> 12) & 63) as f32 / 31.5 - 1.0;
        let l_scale = ((header24 >> 18) & 31) as f32 / 31.0;
        let has_alpha = (header24 >> 23) != 0;
        let p_scale = f32::from((header16 >> 3) & 63) / 63.0;
        let q_scale = f32::from((header16 >> 9) & 63) / 63.0;
        let is_landscape = (header16 >> 15) != 0;
        let l_max = if has_alpha { 5 } else { 7 };
        let lx = usize::from(3.max(if is_landscape { l_max } else { header16 & 7 }));
        let ly = usize::from(3.max(if is_landscape { header16 & 7 } else { l_max }));
        let (a_dc, a_scale, rest) = if has_alpha {
            let header8 = *hash.get(5)?;
            (f32::from(header8 & 15) / 15.0, f32::from(header8 >> 4) / 15.0, hash.get(6..)?)
        } else {
            (1.0, 1.0, hash.get(5..)?)
        };
        let mut nibbles = rest.iter().flat_map(|byte| [byte & 15, byte >> 4]);
        // saturation boosted by 1.25 to make up for quantization, as the original does
        let mut channel = |nx: usize, ny: usize, scale: f32| -> Option<Vec<(usize, usize, f32)>> {
            let mut ac = Vec::new();
            for cy in 0..ny {
                let mut cx = usize::from(cy == 0);
                while cx * ny < nx * (ny - cy) {
                    ac.push((cx, cy, (f32::from(nibbles.next()?) / 7.5 - 1.0) * scale));
                    cx += 1;
                }
            }
            Some(ac)
        };
        let l_ac = channel(lx, ly, l_scale)?;
        let p_ac = channel(3, 3, p_scale * 1.25)?;
        let q_ac = channel(3, 3, q_scale * 1.25)?;
        let a_ac = if has_alpha { channel(5, 5, a_scale)? } else { Vec::new() };
        let ratio = Self::ratio(hash)?;
        let (w, h) = if ratio > 1.0 {
            (DRAWN as usize, (DRAWN / ratio).round() as usize)
        } else {
            ((DRAWN * ratio).round() as usize, DRAWN as usize)
        };
        let mut rgba = Vec::with_capacity(w * h * 4);
        let sum = |ac: &[(usize, usize, f32)], fx: &[f32], fy: &[f32]| -> f32 {
            ac.iter()
                .map(|&(cx, cy, f)| f * fx.get(cx).copied().unwrap_or(0.0) * (fy.get(cy).copied().unwrap_or(0.0) * 2.0))
                .fold(0.0, |total, term| total + term)
        };
        for y in 0..h {
            let fy: Vec<f32> = (0..7).map(|cy| libm::cosf(PI / h as f32 * (y as f32 + 0.5) * cy as f32)).collect();
            for x in 0..w {
                let fx: Vec<f32> = (0..7).map(|cx| libm::cosf(PI / w as f32 * (x as f32 + 0.5) * cx as f32)).collect();
                let l = l_dc + sum(&l_ac, &fx, &fy);
                let p = p_dc + sum(&p_ac, &fx, &fy);
                let q = q_dc + sum(&q_ac, &fx, &fy);
                let a = a_dc + sum(&a_ac, &fx, &fy);
                let b = l - 2.0 / 3.0 * p;
                let r = (3.0 * l - b + q) / 2.0;
                let g = r - q;
                rgba.extend([r, g, b, a].map(|c| (c.clamp(0.0, 1.0) * 255.0) as u8));
            }
        }
        Some((w, h, rgba))
    }

    /// The approximate aspect ratio of the image hashed.
    fn ratio(hash: &[u8]) -> Option<f32> {
        let has_alpha = (hash.get(2)? & 0x80) != 0;
        let l_max = if has_alpha { 5 } else { 7 };
        let l_min = hash.get(3)? & 7;
        let is_landscape = (hash.get(4)? & 0x80) != 0;
        let (lx, ly) = if is_landscape { (l_max, l_min) } else { (l_min, l_max) };
        Some(f32::from(lx) / f32::from(ly))
    }
}

impl AsRef<[u8]> for Thumbhash {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
