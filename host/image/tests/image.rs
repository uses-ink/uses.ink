//! The image work on generated fixtures (`fixture/`, each made for one case and small): every
//! fixture's outcome against its golden, which holds the hash of every copy, so a machine that
//! made other bytes fails it; then each rule on its own.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

extern crate uses_ink_host_image as host_image;

use std::fmt::Write as _;
use std::path::Path;

use engine::Images;
use engine::image::{Encoded, Kept, Prepared, Source};
use host_image::{Pixels, Reason, Refused, Settings};
use model::{ContentPath, Diagnostic, Dimensions, Hash, MediaType};

const FIXTURE: &str = "fixture";

fn bytes(name: &str) -> Vec<u8> {
    std::fs::read(Path::new(FIXTURE).join(name)).unwrap()
}

fn path(name: &str) -> ContentPath {
    name.parse().unwrap()
}

fn prepare(pixels: &Pixels, name: &str) -> Result<Prepared, Refused> {
    let (file, bytes) = (path(name), bytes(name));
    let media = MediaType::try_from(&file).unwrap();
    let source = Source::new(&file, media, Hash::of(&bytes), &bytes);
    pixels.prepare(&[source]).pop().unwrap()
}

fn varied(name: &str) -> engine::image::Variants {
    match prepare(&Pixels::default(), name).unwrap() {
        Prepared::Varied(variants) => variants,
        other => panic!("{name} was not varied: {other:?}"),
    }
}

fn line(encoded: &Encoded) -> String {
    let Dimensions { width, height, .. } = encoded.dimensions;
    format!("{} {width}x{height} {} bytes {}", encoded.media.essence(), encoded.bytes.len(), Hash::of(&encoded.bytes))
}

fn report(name: &str) -> String {
    let mut out = String::new();
    match prepare(&Pixels::default(), name) {
        Ok(Prepared::Kept { dimensions, reason }) => {
            let _infallible = writeln!(out, "kept {}x{} {reason:?}", dimensions.width, dimensions.height);
        }
        Ok(Prepared::Varied(variants)) => {
            let Dimensions { width, height, .. } = variants.dimensions;
            let _infallible = writeln!(out, "varied {width}x{height}");
            for copy in &variants.copies {
                let _infallible = writeln!(out, "copy {}", line(copy));
            }
            let placeholder = variants.placeholder.as_ref().map_or_else(|| "none".to_owned(), line);
            let _infallible = writeln!(out, "placeholder {placeholder}");
        }
        Ok(other) => panic!("{other:?}"),
        Err(refused) => {
            let diagnostic = Diagnostic::from(refused);
            let _infallible = writeln!(out, "refused {} on {:?}", diagnostic.code(), diagnostic.subject);
        }
    }
    out
}

#[test]
fn every_fixture_prepares_as_its_golden_says_image() {
    let mut names: Vec<String> = std::fs::read_dir(FIXTURE)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| Path::new(name).extension().is_none_or(|extension| extension != "snap"))
        .collect();
    names.sort();
    let cases: Vec<(String, String)> = names.iter().map(|name| (name.replace('.', "-"), report(name))).collect();
    fixture::cases(FIXTURE, cases);
}

#[test]
fn a_decompression_bomb_is_refused_from_its_header_image() {
    let refused = prepare(&Pixels::default(), "bomb.png").unwrap_err();
    assert!(matches!(refused.reason, Reason::Budget { width: 100_000, height: 100_000, limit: 50_000_000 }));
    let diagnostic = Diagnostic::from(refused);
    assert_eq!(diagnostic.code(), "image.budget.pixels");
}

#[test]
fn orientation_is_applied_and_metadata_is_left_behind_image() {
    let variants = varied("orientation.jpg");
    assert_eq!(variants.dimensions, Dimensions::new(32, 64), "turned a quarter clockwise");
    let jpeg = variants.copies.iter().find(|copy| copy.media == MediaType::Jpeg).unwrap();
    let shown = image::load_from_memory(&jpeg.bytes).unwrap().into_rgb8();
    let (top, bottom) = (shown.get_pixel(16, 8).0, shown.get_pixel(16, 56).0);
    assert!(top[0] > 180 && top[2] < 80, "the red left half is on top: {top:?}");
    assert!(bottom[2] > 180 && bottom[0] < 80, "the blue right half is below: {bottom:?}");
    for copy in &variants.copies {
        let has = |needle: &[u8]| copy.bytes.windows(needle.len()).any(|window| window == needle);
        assert!(!has(b"Exif") && !has(b"II*\0"), "{} keeps EXIF", copy.media);
    }
}

#[test]
fn a_cmyk_jpeg_is_shown_in_its_colours_image() {
    let variants = varied("cmyk.jpg");
    let jpeg = variants.copies.iter().find(|copy| copy.media == MediaType::Jpeg).unwrap();
    let [r, g, b] = image::load_from_memory(&jpeg.bytes).unwrap().into_rgb8().get_pixel(16, 16).0;
    assert!(r < 80 && g > 180 && b > 180, "cyan: {:?}", [r, g, b]);
}

#[test]
fn a_transparent_png_keeps_its_alpha_and_has_no_placeholder_image() {
    let variants = varied("clear.png");
    let media: Vec<MediaType> = variants.copies.iter().map(|copy| copy.media).collect();
    assert_eq!(media, [MediaType::Avif, MediaType::Png]);
    assert!(variants.placeholder.is_none());
    let png = image::load_from_memory(&variants.copies[1].bytes).unwrap().into_rgba8();
    assert_eq!(png.get_pixel(0, 0).0[3], 0);
    assert_eq!(png.get_pixel(24, 16).0[3], 255);
}

#[test]
fn an_animated_gif_is_kept_as_it_is_image() {
    let prepared = prepare(&Pixels::default(), "animated.gif").unwrap();
    assert_eq!(prepared, Prepared::Kept { dimensions: Dimensions::new(16, 16), reason: Kept::Animated });
}

#[test]
fn the_same_bytes_give_the_same_copies_image() {
    let first = prepare(&Pixels::default(), "gradient.jpg").unwrap();
    let again = prepare(&Pixels::default(), "gradient.jpg").unwrap();
    assert_eq!(first, again);
}

#[test]
fn an_image_is_prepared_once_per_key_image() {
    let pixels = Pixels::default();
    let first = prepare(&pixels, "gradient.jpg").unwrap();
    let start = std::time::Instant::now();
    assert_eq!(prepare(&pixels, "gradient.jpg").unwrap(), first);
    assert!(start.elapsed().as_millis() < 5, "taken from what was made");
}

#[test]
fn the_ladder_never_grows_an_image() {
    let settings = Settings::default();
    assert_eq!(settings.ladder(4000), [480, 960, 1440]);
    assert_eq!(settings.ladder(1500), [480, 960, 1440]);
    assert_eq!(settings.ladder(1000), [480, 1000]);
    assert_eq!(settings.ladder(500), [500]);
    assert_eq!(settings.ladder(300), [300]);
}
