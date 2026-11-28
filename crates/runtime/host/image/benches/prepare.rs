//! The image work on photo-sized input: one and ten generated 4000 by 3000 JPEGs, each a
//! smooth gradient under seeded noise, so the encoders see detail at every scale. With
//! `INK_BENCH_PHOTOS` naming a folder of JPEGs, those are used instead.
//! `cargo bench -p uses-ink-host-image`.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "a bench stops at the first surprise")]

extern crate uses_ink_host_image as host_image;

use divan::Bencher;
use engine::Images;
use engine::image::Source;
use host_image::Pixels;
use model::{ContentPath, Hash, MediaType};

fn main() {
    divan::main();
}

/// A 4000 by 3000 JPEG drawn from `seed`.
fn photo(seed: u32) -> Vec<u8> {
    let (width, height) = (4000u16, 3000u16);
    let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    let mut pixels = Vec::with_capacity(usize::from(width) * usize::from(height) * 3);
    for y in 0..u32::from(height) {
        for x in 0..u32::from(width) {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let noise = u8::try_from(state & 31).unwrap();
            let shade = |value: u32| u8::try_from(value).unwrap();
            let base = [shade(x * 200 / 4000), shade(y * 200 / 3000), shade((x + y) * 100 / 7000)];
            pixels.extend(base.map(|channel| channel.saturating_add(noise)));
        }
    }
    let mut bytes = Vec::new();
    jpeg_encoder::Encoder::new(&mut bytes, 90).encode(&pixels, width, height, jpeg_encoder::ColorType::Rgb).unwrap();
    bytes
}

/// `count` photos: from `INK_BENCH_PHOTOS`, or generated.
fn photos(count: usize) -> Vec<(ContentPath, Vec<u8>)> {
    let given: Vec<Vec<u8>> = std::env::var_os("INK_BENCH_PHOTOS").map_or_else(Vec::new, |dir| {
        let mut paths: Vec<_> = std::fs::read_dir(dir).unwrap().map(|entry| entry.unwrap().path()).collect();
        paths.sort();
        paths
            .into_iter()
            .filter(|path| path.extension().is_some_and(|e| e == "jpg"))
            .map(|path| std::fs::read(path).unwrap())
            .collect()
    });
    (0..count)
        .map(|at| {
            let bytes = given.get(at).cloned().unwrap_or_else(|| photo(u32::try_from(at).unwrap()));
            (format!("photo{at}.jpg").parse().unwrap(), bytes)
        })
        .collect()
}

#[divan::bench(args = [1, 10], sample_count = 2, sample_size = 1)]
fn prepare(bencher: Bencher<'_, '_>, count: usize) {
    let held = photos(count);
    let sources: Vec<Source<'_>> =
        held.iter().map(|(file, bytes)| Source::new(file, MediaType::Jpeg, Hash::of(bytes), bytes)).collect();
    // a fresh `Pixels` each time, so nothing is taken from an earlier sample
    bencher.bench(|| Pixels::default().prepare(&sources));
}
