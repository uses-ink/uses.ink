## 0033 The pen crate is `freehand`; brand drawings are data

Accepted, owner. The pen of 0025 is the crate `freehand` at `freehand/` (MIT OR Apache-2.0, `no_std`,
only `libm`). The logo, mark and favicon are described in `brand/figure.toml` (named polygons,
shapes, weights, colours by token name) and drawn by `cargo xtask brand`; the SVG files are written
with `unspliced`, whose bare `xmlns` on `svg` writes the namespace a standalone file needs.
**Why:** one implementation of the pen; geometry is data, so a drawing changes without code.
**Reversal:** none planned.
