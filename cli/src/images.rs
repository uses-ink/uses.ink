//! The host's image work, reporting progress: the engine hands every image to the host in one call,
//! so this prepares them in batches the size of the machine's parallelism and tells the output
//! layer between batches how many are done (`docs/cli.md` section 8).

use std::cell::RefCell;

use engine::Images;
use engine::image::{Prepared, Source};

use crate::error::Error;
use crate::output::Io;
use crate::term::Phase;

/// Image work that says how far it has got.
pub(crate) struct Watched<'a, 'b, I> {
    images: &'a I,
    io: RefCell<&'a mut Io<'b>>,
    batch: usize,
    /// The first progress that could not be written, reported when the render is over.
    failed: RefCell<Option<Error>>,
}

impl<'a, 'b, I> Watched<'a, 'b, I> {
    /// `images`, reporting to `io`, in batches of as many images as the machine has threads.
    pub(crate) fn new(images: &'a I, io: &'a mut Io<'b>) -> Self {
        let batch = std::thread::available_parallelism().map_or(1, usize::from);
        Self { images, io: RefCell::new(io), batch, failed: RefCell::new(None) }
    }

    /// The end of the render: the error of a progress line that could not be written, if there was
    /// one.
    pub(crate) fn finish(self) -> Result<(), Error> {
        self.failed.into_inner().map_or(Ok(()), Err)
    }

    /// Keeps the first error of reporting progress.
    fn keep(&self, written: Result<(), Error>) {
        if let Err(error) = written {
            let mut failed = self.failed.borrow_mut();
            failed.get_or_insert(error);
        }
    }
}

impl<I: Images> Images for Watched<'_, '_, I> {
    type Refusal = I::Refusal;

    fn prepare(&self, sources: &[Source<'_>]) -> Vec<Result<Prepared, Self::Refusal>> {
        let Ok(mut io) = self.io.try_borrow_mut() else { return self.images.prepare(sources) };
        let units = |count: usize| u64::try_from(count).unwrap_or(u64::MAX);
        io.begin(Phase::Rendering, units(sources.len()), "images");
        let mut prepared = Vec::with_capacity(sources.len());
        for batch in sources.chunks(self.batch.max(1)) {
            prepared.extend(self.images.prepare(batch));
            let item = batch.last().map(|source| source.file.as_str());
            self.keep(io.tick(units(prepared.len()), item));
        }
        self.keep(io.end());
        prepared
    }
}
