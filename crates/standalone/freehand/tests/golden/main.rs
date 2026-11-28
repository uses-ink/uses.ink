//! The pen's goldens, compared with `fixture/<name>.snap` by the fixture harness: the same bytes
//! natively and on wasm (`cargo test -p freehand --target wasm32-wasip2`). `INSTA_UPDATE=always`
//! rewrites them, natively only, and fails the run until they are read.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

mod drawing;
mod marks;

#[test]
fn goldens() {
    fixture::cases("fixture", [("golden", drawing::drawing()), ("marks", marks::drawing())]);
}
