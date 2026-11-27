//! The adapter.

use std::collections::BTreeSet;
use std::time::Instant;

use blob::Blob;
use build::{Build, Built, Event, Events, Outcome, Request, Source, Status};
use engine::{BaseUrl, Blobs};
use model::Bytes;

use crate::pipeline::{Made, Pipeline};
use crate::Error;

type Failure<E> = build::Error<Error<E>>;

/// The local builder, storing deploys in `B`.
#[derive(Debug)]
pub struct Local<B> {
    blobs: B,
}

impl<B: Blob> Local<B> {
    /// A builder that stores the blobs of every deploy in `blobs`.
    pub const fn new(blobs: B) -> Self {
        Self { blobs }
    }

    /// Stores every blob the manifest names, each once, under the request's tenant.
    async fn store(&self, request: &Request, made: Made) -> Result<Built, Failure<B::Error>> {
        let mut seen = BTreeSet::new();
        let mut bytes = 0_u64;
        for entry in made.manifest.entries() {
            let hash = entry.blob();
            let body = match made.blobs.get(&hash) {
                Some(body) => std::borrow::Cow::Borrowed(body.as_slice()),
                None => made.store.read(&hash).map_err(|_| Self::fail(Error::Missing { blob: hash }))?,
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
        let Source::Folder(dir) = &request.source else {
            return Err(build::Error::Unsupported { kind: request.source.kind() });
        };
        let base: BaseUrl = request.base.as_str().parse().map_err(|source| Self::fail(Error::Base { source }))?;
        let made = Pipeline { dir, base }.run(events).map_err(|id| Self::fail(Error::Format { id }))?;
        let Some(made) = made else { return Ok(Outcome::Refused) };
        let built = self.store(request, made).await?;
        events.event(Event::Rendered { files: built.manifest.entries().len() as u64, bytes: built.bytes });
        Ok(Outcome::Built(built))
    }
}

impl<B: Blob> Build for Local<B> {
    type Error = Error<B::Error>;

    async fn run(&self, request: &Request, events: &mut impl Events) -> Result<Outcome, Failure<B::Error>> {
        let started = Instant::now();
        events.event(Event::Started { build: request.build, trigger: request.trigger });
        let result = self.attempt(request, events).await;
        let status = match &result {
            Ok(Outcome::Built(_)) => Status::Succeeded,
            Ok(_) => Status::Refused,
            Err(_) => Status::Failed,
        };
        events.event(Event::Finished { status, wall: started.elapsed() });
        result
    }
}
