//! The adapter.

use std::collections::BTreeSet;
use std::time::Instant;

use blob::Blob;
use build::{Build, Built, Event, Events, Outcome, Request, Source, Status};
use engine::{BaseUrl, Blobs};
use host_native::Memory;
use logging::{Field as _, Instrument as _};
use model::Bytes;

use crate::Error;
use crate::bundle::Bundle;
use crate::pipeline::{Made, Pipeline};
use crate::remote::{Offline, Remote};

type Failure<E> = build::Error<Error<E>>;

/// The in-process builder, storing deploys in `B`.
#[derive(Debug)]
pub struct Local<B, R = Offline> {
    blobs: B,
    remote: R,
}

impl<B: Blob> Local<B> {
    /// A builder that stores the blobs of every deploy in `blobs` and reads only its own disk.
    pub const fn new(blobs: B) -> Self {
        Self { blobs, remote: Offline }
    }

    /// The same builder, reading a git source through `remote`.
    pub fn with_remote<R: Remote>(self, remote: R) -> Local<B, R> {
        Local { blobs: self.blobs, remote }
    }
}

impl<B: Blob, R: Remote> Local<B, R> {
    /// Stores every blob the manifest names, each once, under the request's tenant.
    async fn store(&self, request: &Request, made: Made) -> Result<Built, Failure<B::Error>> {
        let mut seen = BTreeSet::new();
        let mut bytes = 0_u64;
        for entry in made.manifest.entries() {
            let hash = entry.blob();
            let body = match made.blobs.get(&hash) {
                Some(body) => std::borrow::Cow::Borrowed(body.as_slice()),
                None => made.store.read(&hash).map_err(|source| Self::fail(Error::Missing { blob: hash, source }))?,
            };
            bytes = bytes.saturating_add(body.len() as u64);
            if seen.insert(hash) {
                self.blobs.put(&request.tenant, hash, [body]).await.map_err(|error| Self::fail(Error::Blob(error)))?;
            }
        }
        Ok(Built { manifest: made.manifest, bytes: Bytes(bytes) })
    }

    fn fail(error: Error<B::Error>) -> Failure<B::Error> {
        build::Error::Backend { source: error }
    }

    /// Everything of a build but its first and last event.
    async fn attempt(&self, request: &Request, events: &mut impl Events) -> Result<Outcome, Failure<B::Error>> {
        let mut store = Memory::default();
        let fetching = async {
            match &request.source {
                Source::Folder(dir) => Ok(Pipeline::read(dir, &mut store)),
                Source::Git { .. } if self.remote.reaches() => {
                    let fetched = self.remote.fetch(request, &mut store).await;
                    fetched.map_err(|source| Self::fail(Error::Remote(Box::new(source))))
                }
                Source::Bundle(tree) => {
                    let bundle = Bundle { blobs: &self.blobs, tenant: &request.tenant };
                    let read = bundle.read(*tree, &mut store).await.map_err(Self::fail)?;
                    Ok(Ok(read))
                }
                other => Err(build::Error::Unsupported { kind: other.kind() }),
            }
        };
        let read = fetching.instrument(logging::stage!(logging::Stage::Fetch, "snapshot")).await?;
        let base: BaseUrl = request.base.as_str().parse().map_err(|source| Self::fail(Error::Base { source }))?;
        let made = Pipeline { base }.run(read, store, events).map_err(|id| Self::fail(Error::Format { id }))?;
        let Some(made) = made else { return Ok(Outcome::Refused) };
        let built = self.store(request, made).instrument(logging::stage!(logging::Stage::Write, "blobs")).await?;
        events.event(Event::Rendered { files: built.manifest.entries().len() as u64, bytes: built.bytes });
        Ok(Outcome::Built(built))
    }
}

impl<B: Blob, R: Remote> Build for Local<B, R> {
    type Error = Error<B::Error>;

    async fn run(&self, request: &Request, events: &mut impl Events) -> Result<Outcome, Failure<B::Error>> {
        let started = Instant::now();
        events.event(Event::Started { build: request.build, trigger: request.trigger });
        let span = logging::build!(request.build, request.trigger);
        span.in_scope(
            || logging::info!(name: logging::event::BUILD_START, trigger = logging::Kind::kind(request.trigger)),
        );
        let result = self.attempt(request, events).instrument(span.clone()).await;
        let status = match &result {
            Ok(Outcome::Built(_)) => Status::Succeeded,
            Ok(_) => Status::Refused,
            Err(_) => Status::Failed,
        };
        span.in_scope(|| {
            let took = logging::Took(started.elapsed()).field();
            if let Err(error) = &result {
                logging::error!(name: logging::event::BUILD_END, status = logging::Kind::kind(status), took_us = took, error = logging::Failure(error).field());
            } else {
                logging::info!(name: logging::event::BUILD_END, status = logging::Kind::kind(status), took_us = took);
            }
        });
        events.event(Event::Finished { status, wall: started.elapsed() });
        result
    }
}
