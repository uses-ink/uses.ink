---
number: "0040"
title: "Images are prepared by the host, deterministically, and served as hashed copies"
status: accepted
date: 2026-11-26
area: engine
supersedes: []
---

## 0040 Images are prepared by the host, deterministically, and served as hashed copies

Accepted. The engine decides which files get copies, their routes, headers and what a theme
learns of them; the host does the pixel work behind `engine::Images`, as with highlighting
(0039), in `crates/runtime/host/image` (`uses-ink-host-image`). Before any page is rendered, each raster asset
is decoded within a pixel budget (50 million by default; a header over it is
`image.budget.pixels`, refused before a pixel is decoded), its EXIF orientation applied, resized
with Catmull-Rom to the widths 480, 960 and 1440 that are narrower than it (a width within a tenth of the image's own is replaced by the
image's own width when the image is narrower than the widest step; never wider), and encoded as AVIF (ravif, quality 70, speed 9) and JPEG (quality 80,
baseline with optimized tables), or PNG when it has transparency. Re-encoding leaves every bit of metadata behind,
so the original is not served; the images kept as they are (below) are, metadata included, and
each notice says so. Each copy is served at `<name>.<width>.<hash>.<ext>` with
`cache-control: public, max-age=31536000, immutable`; the asset carries a `model::Picture` (its
size, its copies, its placeholder) for the theme's `width`, `height`, `srcset` and `<picture>`.
The placeholder is the image's thumbhash drawn as a 32-pixel PNG, served as a hashed file and
shown under the image, not inlined: `unspliced` refuses a `data:` URL at run time and
`style-src 'self'` refuses inline styles, so the CSP stays as it is. Animated GIF and WebP are
served as they are (no reader page runs script, so a poster could not start them), and AVIF is,
since decoding it takes dav1d (C); each is a notice.

Determinism, measured: ravif cuts as many tiles as rayon has threads, so its bytes followed the
core count; the work runs rav1e on one thread (one tile) without assembly, jpeg-encoder without
its SIMD feature, `fast_image_resize`'s SIMD paths give the scalar path's bytes for this filter,
and the ported thumbhash takes its cosines from `libm`. The goldens in `crates/runtime/host/image/fixture` hold
the hash of every copy. A `Key` (the source's hash and media type, the settings, the pixel limit, and the version of
every registry crate the image crate reaches through `Cargo.lock`) names one image's work; `host_image::Pixels` keeps what it made by key, and keeps it across builds
in an `engine::Cache` (as measured, below). **Why:** layout stability and small pages for readers,
no metadata leaks, no C. **Reversal:** encode time on hosted builders (a 4000 by 3000 photo
costs about 5 s of one core at speed 9) pushes builds past their budget: speed 10 again, or
fewer widths.

As measured (2026-11-27, performance pass; ten 4000 by 3000 photos cut from one wallpaper, 8
workers, release build, on a 14-thread laptop shared with other builds):

| Build | Wall | CPU | Peak memory |
|---|---|---|---|
| 10 photos, before | 7.1 s | 39 s | 733 MB |
| 10 photos, cold cache | 5.1 s | 37 s | 564 MB |
| 10 photos, warm cache | 0.37 s | 0.2 s | 45 MB |
| 1 photo, before | 2.6 s | 2.5 s | 111 MB |
| 1 photo, cold cache | 1.7 s | 2.4 s | 122 MB |

Every build gives the same deploy, byte for byte. Three changes. **The cache:** an
`engine::Cache` port (typed `cache::Key` of a `Kind` and a digest, values as bytes), the first
piece of the render unit index; `host_native::Disk` keeps it in a folder, `--cache <dir>` or
`<site>/.ink/cache` by default, so `rm -r .ink` resets everything ink owns and a hosted
builder's cache stays per tenant. Values are files named by their BLAKE3 hash, each key an
entry listing its values and ending with the hash of that list; every file is written under a
name of its own and renamed into place. A value or entry that does not check out is removed and
said (`cache.entry.corrupt`) and the image made again; a folder made for the first time says
`render.cache.cold`; past 2 GiB the entries used longest ago go (`cache.entry.evicted`). The
folder holds `.ink-cache`, so the folder walk and the git tree skip it wherever it is
(`source.cache.skipped`). **The work:** an image is decoded on one thread, cut into its widths,
and its full-size pixels dropped; every copy is then a job, the costliest first across all
images, so one photo uses several cores and ten do not wait on the last one's widest AVIF. The
resizer no longer copies the full-size pixels for each width. **The memory:** at most two images
are decoded at once; what is left is mostly eight rav1e encoders at once (a 1440-wide encode
holds some 40 MB), and one decode slot instead of two saved about 3%. The bytes do not follow the
number of threads: a test prepares every fixture on 1, 2, 3 and 8 and compares.

The CLI site tests share one cache under `target/`, so after the first run the blog's photos
are taken from it; their photos (64 by 32 and 48 by 32) were already small enough to encode
within budget cold, so the `HACK(performance pass)` override in `.config/nextest.toml` is gone
and the blog tests run under the highlighting budget they carry anyway. The override also hid
that the test writing pins back built all five sites in one process, 300 ms beside the rest of
the suite even with every photo cached; it is one test per site now.

Not changed, since each changes what readers get; each measured on one photo, CPU time on one
core, SSIM against the resized original:

| Change | Cost | Bytes | SSIM |
|---|---|---|---|
| AVIF speed 9 instead of 10 (taken, below) | +40% to +55% | -13% to -15% | 0.932 to 0.935 at 1440, 0.924 to 0.942 at 480 |
| AVIF speed 8 or 6 | x2.6 to x3.9 | -15% to -19% | at most 0.004 over speed 9 |
| AVIF 8-bit instead of 10-bit (ravif's default) | 0% to -19% | +5% to +7% | 0.006 to 0.010 lower |
| JPEG baseline with optimized Huffman tables (taken, below) | same | -16% at 1440, -13% at 480 | identical pixels in libjpeg |
| JPEG progressive with optimized tables | same | -6% to -8% | identical pixels |
| No 1440 copy | -60% of AVIF time | | 3x screens get 960 |

The 1440 AVIF is 1.4 to 1.7 s of a photo's 2.5 s; the 960 is 0.66 s, the 480 0.22 s, every JPEG
together under 0.1 s, the decode 65 ms and the three resizes 130 ms. Speed 9 behind the cache and
optimized Huffman tables are the two worth taking; jpeg-encoder's progressive mode makes files
larger, not smaller, and the placeholder already covers what progressive loading would show.

Speed 9 is taken (2026-11-27), behind the cache. Measured on the same machine, 8 workers,
release build, but on generated photos (ten seeded `magick plasma:fractal` images, 4000 by 3000,
with noise), since the wallpaper is not in the repo:

| Build | Wall | CPU | Peak memory | AVIF bytes, 30 copies |
|---|---|---|---|---|
| 10 photos, speed 10, cold cache | 5.1 to 5.6 s | 39 to 42 s | 586 MB | 1,022,036 |
| 10 photos, speed 9, cold cache | 8.7 to 8.9 s | 65 to 67 s | 644 MB | 871,687 (-14.7%) |
| 10 photos, either, warm cache | 0.05 s | 0.05 s | 60 MB | |
| 1 photo, speed 10, cold cache | 1.8 to 2.1 s | 2.7 to 3.0 s | 124 MB | |
| 1 photo, speed 9, cold cache | 3.1 to 3.4 s | 4.6 to 5.2 s | 133 MB | |

A cold build costs about 65% more CPU on these photos, more than the one-core estimate in the
table above; a warm build costs the same. The JPEG copies are unchanged.

Baseline JPEG with optimized Huffman tables is taken, with its own decoder. jpeg-encoder 0.7.1
writes optimized tables only in its sequential mode, one scan per component, and zune-jpeg 0.5.15
(the decoder `image` 0.25 pins) misread such a file with 4:2:0 chroma: against the source,
libjpeg's decode of the 48 by 36 fixture was 31 dB and zune-jpeg's 5 dB, and a 1600 by 1200
photo's copies came out at 8 dB beside libjpeg's at every width. The file is valid and browsers
decode it with libjpeg-turbo. zune-jpeg 0.5.16-rc3 reads it (a 96 by 72 probe: 5.7 dB on 0.5.15,
37.7 dB on rc3, the same as the other three sampling and table modes), so `crates/runtime/host/image` depends on
`zune-jpeg =0.5.16-rc3` directly, turns off `image`'s `jpeg` feature, and reads JPEG itself
(`crates/runtime/host/image/src/jpeg.rs`: size from the headers, EXIF orientation, grey or RGB pixels, CMYK
converted). A prerelease cannot satisfy `image`'s `^0.5` requirement, so no `[patch]` can swap it
in under `image`; that is why the reader is ours. The cache key's codec tag is no longer typed by
hand: `crates/runtime/host/image/build.rs` reads the versions of the crate's registry dependencies from
`Cargo.lock`. The fixtures' JPEG copies shrink (773 to 310 bytes, 1074 to 599) since baseline
carries less overhead than the progressive files they replace. **Reversal:** zune-jpeg 0.5.16
is released and `image` accepts it: drop the direct reader and turn the feature back on.
