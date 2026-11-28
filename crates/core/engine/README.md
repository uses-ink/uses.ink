# uses-ink-engine

The engine turns a snapshot of a folder into a site, and a site into the bytes of a deploy. It
holds the decisions of what a source means (which file is a page, which link points where) and the
rendering of the result. It is pure: nothing in it reads a clock, the environment, the filesystem
or randomness (decision 0006), so the same snapshot gives the same deploy on any host.

## What it may use

Only `uses-ink-model`, `uses-ink-diag`, `uses-ink-config` and `uses-ink-logging`. It never names a
concrete blob store, format, theme or image library. The host hands them in through the traits of
`crates/core/engine/src/host.rs` (`Blobs`, `Pins`, `Format`, `Theme`) and
`crates/core/engine/src/image.rs` (`Images`), and a part a call does not need can be `()`. The
list for every crate is in [the crate index](../../../docs/crates.md).

## Where things are

- `Engine` is in `crates/core/engine/src/lib.rs`: the four parts it is generic over, with
  `interpret` and `render` as methods.
- Interpreting is `crates/core/engine/src/interpret/`: the pipeline stages are under
  `crates/core/engine/src/interpret/pipeline/`. Each decision is a ladder of witnesses; the first
  that testifies wins and is recorded with its evidence.
- Rendering is `crates/core/engine/src/render/`: one file per artifact (feeds, sitemap, pictures,
  assets, the theme's pages). `Unit` and its keys name what a cached piece of work was made from.
- `Compared` and `Understood` in `crates/core/engine/src/change.rs` say what changed against the
  previous build, and why (decision 0042).

## How to test it

The tests are `crates/core/engine/tests/engine/`, one file per concern, over the site fixtures in
`crates/core/engine/fixture/`: a fixture is a folder with its expected output beside it, compared
through insta. Run them on the farm, never locally:

    farm cargo nextest run -p uses-ink-engine

To accept a changed golden after reading its diff, use `farm bless`.
