//! A format component that misbehaves on purpose, so `host/wasmtime`'s tests can check that
//! every misbehaviour becomes a typed refusal and the host serves the next call. What it does is
//! chosen by the file's stem (conventions, Tests: hostile input):
//!
//! - `spin`: loops forever;
//! - `grow`: allocates without end;
//! - `deep`: recurses until its stack overflows;
//! - `panic`: panics, which aborts;
//! - `garbage`: returns a document whose body names a block that does not exist;
//! - anything else: an empty, well-formed read.

mod bind;

use std::hint::black_box;

use sdk::bind::ink::model::document::{Block, Document};
use sdk::bind::ink::model::read::{Front, Read};

/// The misbehaving format.
pub(crate) struct Hostile;

impl Hostile {
    fn spin() -> ! {
        let mut turns: u64 = 0;
        loop {
            turns = black_box(turns.wrapping_add(1));
        }
    }

    fn grow() -> usize {
        let mut held: Vec<Vec<u8>> = Vec::new();
        loop {
            held.push(vec![1; 1 << 20]);
            black_box(&held);
        }
    }

    #[expect(unconditional_recursion, reason = "overflowing its stack is what this hostile plugin is for")]
    fn deep(depth: u64) -> u64 {
        let frame = black_box([depth; 64]);
        Self::deep(depth + 1).wrapping_add(frame[0])
    }

    fn read(body: Vec<u32>, blocks: Vec<Block>) -> Read {
        Read {
            document: Document { blocks, inlines: Vec::new(), body, footnotes: Vec::new() },
            front: Front {
                title: None,
                date: None,
                updated: None,
                slug: None,
                tags: None,
                publish: None,
                draft: None,
                kind: None,
                description: None,
                language: None,
                block: None,
            },
            heading: None,
            diagnostics: Vec::new(),
            notices: Vec::new(),
        }
    }
}

impl bind::exports::format::Guest for Hostile {
    fn handles(_path: String) -> bool {
        true
    }

    #[expect(clippy::panic, reason = "panicking is what this hostile plugin is for")]
    fn read(path: String, _bytes: Vec<u8>) -> Read {
        match path.strip_suffix(".md").unwrap_or(&path) {
            "spin" => Self::spin(),
            "grow" => Self::read(vec![u32::try_from(Self::grow()).unwrap_or(0)], Vec::new()),
            "deep" => Self::read(vec![u32::try_from(Self::deep(0)).unwrap_or(0)], Vec::new()),
            "panic" => panic!("asked to"),
            "garbage" => Self::read(vec![7], vec![Block::Rule]),
            _ => Self::read(Vec::new(), Vec::new()),
        }
    }
}
