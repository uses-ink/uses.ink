//! One call's image work spread over a few threads, a copy at a time. An image is opened
//! (decoded, its placeholder drawn, resized to every width of its ladder) on one thread, and
//! its full-size pixels dropped as soon as the copies are cut; each copy is then encoded as its
//! own job, the costliest first across every image, so one large photo keeps every thread busy
//! and ten photos do not wait on the last one's widest copy. Images are opened ahead of the
//! encoding, but at most [`crate::Limits::opening`] at once, which bounds the memory to a couple of
//! full-size images, the cut copies (some 7 MB a photo) and the encoders at work.
//!
//! The order the jobs finish in changes no byte: each copy lands in its own slot, and an image
//! is put together from its slots in ladder order.

use logging::Field as _;
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::sync::{Arc, Condvar, Mutex, PoisonError};

use engine::image::{Encoded, Prepared, Source, Variants};
use model::Dimensions;

use crate::Pixels as Work;
use crate::decode::{Decoded, Pixels};
use crate::error::{Reason, Refused, Step};

/// The work over one call's sources.
pub(crate) struct Run<'a> {
    work: &'a Work,
    sources: &'a [Source<'a>],
    state: Mutex<State>,
    wake: Condvar,
}

/// Where the work stands.
#[derive(Default)]
struct State {
    /// The next source to open.
    next: usize,
    /// Images being opened now.
    opening: usize,
    /// Jobs being encoded now.
    encoding: usize,
    /// Copies waiting, the costliest first.
    jobs: BinaryHeap<Job>,
    images: Vec<Image>,
}

/// One source's progress.
enum Image {
    Waiting,
    Open(Open),
    Done(Result<Prepared, Refused>),
}

/// An image whose copies are being encoded.
struct Open {
    dimensions: Dimensions,
    placeholder: Option<Encoded>,
    /// One slot per copy, in ladder order: AVIF then the fallback, at each width.
    slots: Vec<Option<Result<Encoded, Reason>>>,
    left: usize,
}

/// One copy to encode.
struct Job {
    image: usize,
    slot: usize,
    copy: Arc<Pixels>,
}

impl Job {
    /// What it is weighed by: the copy's pixels, an AVIF (even slot) costing some twenty times a
    /// JPEG or PNG of the same size; then the earlier image and slot, so ties go in order.
    fn weight(&self) -> (u64, Reverse<usize>, Reverse<usize>) {
        let pixels = u64::from(self.copy.width) * u64::from(self.copy.height);
        let cost = if self.slot.is_multiple_of(2) { pixels * 20 } else { pixels };
        (cost, Reverse(self.image), Reverse(self.slot))
    }
}

impl PartialEq for Job {
    fn eq(&self, other: &Self) -> bool {
        self.weight() == other.weight()
    }
}

impl Eq for Job {}

impl PartialOrd for Job {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Job {
    fn cmp(&self, other: &Self) -> Ordering {
        self.weight().cmp(&other.weight())
    }
}

/// What a thread takes next.
enum Task {
    Open(usize),
    Encode(Job),
}

/// What opening an image gave.
enum Opened {
    Done(Result<Prepared, Refused>),
    Cut { dimensions: Dimensions, placeholder: Option<Encoded>, copies: Vec<Arc<Pixels>> },
}

impl<'a> Run<'a> {
    pub(crate) fn new(work: &'a Work, sources: &'a [Source<'a>]) -> Self {
        let images = sources.iter().map(|_| Image::Waiting).collect();
        Self { work, sources, state: Mutex::new(State { images, ..State::default() }), wake: Condvar::new() }
    }

    /// Every source's result, in order, made on `threads` threads.
    pub(crate) fn finish(self, threads: usize) -> Vec<Result<Prepared, Refused>> {
        // worker threads do not inherit the caller's span: hand it over
        let parent = logging::Span::current();
        std::thread::scope(|scope| {
            for _ in 0..threads.max(1) {
                scope.spawn(|| parent.in_scope(|| self.serve()));
            }
        });
        let state = self.state.into_inner().unwrap_or_else(PoisonError::into_inner);
        state
            .images
            .into_iter()
            .filter_map(|image| if let Image::Done(done) = image { Some(done) } else { None })
            .collect()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// One thread's loop: an image to open while fewer than [`crate::Limits::opening`] are, so every
    /// copy is known early and the costliest go first; else a copy to encode; else a wait.
    fn serve(&self) {
        while let Some(task) = self.take() {
            match task {
                Task::Open(image) => {
                    let opened =
                        Step::Open.guard(|| self.open(image)).unwrap_or_else(|reason| self.refused(image, reason));
                    self.opened(image, opened);
                }
                Task::Encode(job) => {
                    let encoded = Step::Encode.guard(|| self.encode(&job)).unwrap_or_else(Err);
                    self.encoded(&job, encoded);
                }
            }
            self.wake.notify_all();
        }
    }

    fn take(&self) -> Option<Task> {
        let mut state = self.lock();
        loop {
            if state.next < self.sources.len() && state.opening < self.work.limits().opening.get() {
                let image = state.next;
                state.next += 1;
                state.opening += 1;
                return Some(Task::Open(image));
            }
            if let Some(job) = state.jobs.pop() {
                state.encoding += 1;
                return Some(Task::Encode(job));
            }
            if state.next >= self.sources.len() && state.opening == 0 && state.encoding == 0 {
                return None;
            }
            state = self.wake.wait(state).unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// The image `image` refused for `reason`: what a panic while opening it comes to.
    fn refused(&self, image: usize, reason: Reason) -> Opened {
        match self.sources.get(image) {
            Some(source) => Opened::Done(Err(Refused::new(source.file, reason))),
            None => Opened::Cut { dimensions: Dimensions::new(0, 0), placeholder: None, copies: Vec::new() },
        }
    }

    /// The image taken from what was made or kept, or decoded and cut into its copies.
    fn open(&self, image: usize) -> Opened {
        let Some(source) = self.sources.get(image) else {
            return Opened::Cut { dimensions: Dimensions::new(0, 0), placeholder: None, copies: Vec::new() };
        };
        let _unit = logging::unit!(logging::Unit::Image, source.file).entered();
        if let Some(prepared) = self.work.taken(source) {
            return Opened::Done(Ok(prepared));
        }
        let settings = self.work.settings();
        let refuse = |reason| Refused::new(source.file, reason);
        let pixels = match Decoded::read(source, self.work.limits()) {
            Err(refused) => return Opened::Done(Err(refused)),
            Ok(Decoded::Kept(dimensions, reason)) => {
                logging::debug!(name: logging::event::IMAGE_DECODED, kept = logging::Kind::kind(reason));
                return Opened::Done(Ok(Prepared::Kept { dimensions, reason }));
            }
            Ok(Decoded::Pixels(pixels)) => pixels,
        };
        logging::debug!(
            name: logging::event::IMAGE_DECODED,
            width = logging::Count(u64::from(pixels.width)).field(),
            bytes = logging::Size::from(source.bytes.len()).field(),
        );
        let cut = || -> Result<Opened, Reason> {
            let placeholder = pixels.placeholder()?;
            let mut copies = Vec::new();
            for width in settings.ladder(pixels.width) {
                logging::trace!(name: logging::event::IMAGE_RESIZED, width = logging::Count(u64::from(width)).field());
                copies.push(Arc::new(pixels.resize(width)?));
            }
            Ok(Opened::Cut { dimensions: pixels.dimensions(), placeholder, copies })
        };
        cut().unwrap_or_else(|reason| Opened::Done(Err(refuse(reason))))
    }

    fn opened(&self, image: usize, opened: Opened) {
        let mut state = self.lock();
        state.opening -= 1;
        let (dimensions, placeholder, copies) = match opened {
            Opened::Done(done) => {
                drop(state);
                return self.done(image, done);
            }
            Opened::Cut { dimensions, placeholder, copies } => (dimensions, placeholder, copies),
        };
        let slots = copies.len() * 2;
        if slots == 0 {
            drop(state);
            return self.done(image, Ok(Prepared::Varied(Variants::new(dimensions, Vec::new(), placeholder))));
        }
        if let Some(entry) = state.images.get_mut(image) {
            *entry =
                Image::Open(Open { dimensions, placeholder, slots: (0..slots).map(|_| None).collect(), left: slots });
        }
        for (at, copy) in copies.into_iter().enumerate() {
            state.jobs.push(Job { image, slot: at * 2, copy: Arc::clone(&copy) });
            state.jobs.push(Job { image, slot: at * 2 + 1, copy });
        }
    }

    fn encode(&self, job: &Job) -> Result<Encoded, Reason> {
        let settings = self.work.settings();
        let _unit =
            self.sources.get(job.image).map(|source| logging::unit!(logging::Unit::Image, source.file).entered());
        let encoded = if job.slot.is_multiple_of(2) { job.copy.avif(settings) } else { job.copy.fallback(settings) };
        if let Ok(done) = &encoded {
            logging::debug!(
                name: logging::event::IMAGE_ENCODED,
                format = done.media.essence(),
                width = logging::Count(u64::from(done.dimensions.width)).field(),
                bytes = logging::Size::from(done.bytes.len()).field(),
            );
        }
        encoded
    }

    fn encoded(&self, job: &Job, encoded: Result<Encoded, Reason>) {
        let mut state = self.lock();
        let Some(Image::Open(open)) = state.images.get_mut(job.image) else {
            state.encoding -= 1;
            return;
        };
        if let Some(slot) = open.slots.get_mut(job.slot) {
            *slot = Some(encoded);
        }
        open.left -= 1;
        if open.left > 0 {
            state.encoding -= 1;
            return;
        }
        let Some(Image::Open(open)) =
            state.images.get_mut(job.image).map(|image| std::mem::replace(image, Image::Waiting))
        else {
            state.encoding -= 1;
            return;
        };
        drop(state);
        let done = self.assemble(job.image, open);
        self.done(job.image, done);
        self.lock().encoding -= 1;
    }

    /// An image put together from its slots, in ladder order; the first slot that failed
    /// refuses it.
    fn assemble(&self, image: usize, open: Open) -> Result<Prepared, Refused> {
        let Some(source) = self.sources.get(image) else {
            return Ok(Prepared::Varied(Variants::new(open.dimensions, Vec::new(), open.placeholder)));
        };
        let mut copies = Vec::with_capacity(open.slots.len());
        for slot in open.slots.into_iter().flatten() {
            copies.push(slot.map_err(|reason| Refused::new(source.file, reason))?);
        }
        Ok(Prepared::Varied(Variants::new(open.dimensions, copies, open.placeholder)))
    }

    /// Records an image's result, kept for later calls and builds when it was made.
    fn done(&self, image: usize, done: Result<Prepared, Refused>) {
        if let Err(refused) = &done {
            logging::warn!(
                name: logging::event::IMAGE_REFUSED,
                file = logging::Shown(&refused.file).field(),
                error = logging::Failure(refused).field(),
            );
        }
        if let (Ok(prepared), Some(source)) = (&done, self.sources.get(image)) {
            self.work.keep(source, prepared);
        }
        if let Some(entry) = self.lock().images.get_mut(image) {
            *entry = Image::Done(done);
        }
    }
}
