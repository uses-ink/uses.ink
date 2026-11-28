//! R2 puts of a Worker request body whose length the Worker is not told.

use futures_util::StreamExt as _;
use worker::{Bucket, Data, MultipartUpload, Request, Response, Result, UploadedPart};

/// The R2 put of the request body, handed over as it is.
pub struct Direct {
    bucket: Bucket,
}

impl Direct {
    /// A put into `bucket`.
    pub fn new(bucket: Bucket) -> Self {
        Self { bucket }
    }

    /// Hands the body to R2 without reading it: this works when its length is known.
    pub async fn put(&self, req: Request, name: &str) -> Result<Response> {
        let Some(body) = req.inner().body() else { return Response::ok("fail direct: no body\n") };
        let length = req.headers().get("content-length")?.unwrap_or_else(|| "none".to_owned());
        let framing = req.headers().get("transfer-encoding")?.unwrap_or_else(|| "none".to_owned());
        match self.bucket.put(name, Data::ReadableStream(body)).execute().await {
            Ok(Some(object)) => Response::ok(format!("ok direct put {name}: stored {} bytes (content-length {length}, transfer-encoding {framing})\n", object.size())),
            Ok(None) => Response::ok("fail direct: a put with no condition answered nothing\n"),
            Err(error) => Response::ok(format!("no direct put {name}: refused (content-length {length}, transfer-encoding {framing}): {error}\n")),
        }
    }
}

/// What the egress gateway of 0079 does with a blob from a container: hash while taking it, cut it
/// into parts as it grows, store it only if the hash is the one named.
pub struct Gateway {
    bucket: Bucket,
}

impl Gateway {
    /// The size of a part: R2 needs every part but the last to be the same size and at least 5 MiB.
    const PART: usize = 8 << 20;

    /// A gateway into `bucket`.
    pub fn new(bucket: Bucket) -> Self {
        Self { bucket }
    }

    /// `PUT` stores the blob named by its hash; `GET` returns it.
    pub async fn route(&self, req: Request, hash: &str) -> Result<Response> {
        match req.method() {
            worker::Method::Put => self.put(req, hash).await,
            worker::Method::Get => match self.bucket.get(hash).execute().await? {
                Some(object) => match object.body() {
                    Some(body) => Response::from_bytes(body.bytes().await?),
                    None => Response::error("no body", 500),
                },
                None => Response::error("absent", 404),
            },
            _ => Response::error("method", 405),
        }
    }

    async fn put(&self, mut req: Request, named: &str) -> Result<Response> {
        let started = worker::js_sys::Date::now();
        let mut hasher = blake3::Hasher::new();
        let mut buffer: Vec<u8> = Vec::new();
        let mut upload: Option<MultipartUpload> = None;
        let mut parts: Vec<UploadedPart> = Vec::new();
        let (mut total, mut peak) = (0_usize, 0_usize);
        let mut stream = req.stream()?;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            hasher.update(&chunk);
            total += chunk.len();
            buffer.extend_from_slice(&chunk);
            peak = peak.max(buffer.len());
            while buffer.len() >= Self::PART {
                let part: Vec<u8> = buffer.drain(..Self::PART).collect();
                if upload.is_none() {
                    upload = Some(self.bucket.create_multipart_upload(named).execute().await?);
                }
                if let Some(open) = &upload {
                    let number = u16::try_from(parts.len() + 1).unwrap_or(u16::MAX);
                    parts.push(open.upload_part(number, part).await?);
                }
            }
        }
        let found = hasher.finalize().to_hex().to_string();
        if found != named {
            if let Some(open) = &upload {
                open.abort().await?;
            }
            let mut answer = Response::ok(format!("no mismatch: the bytes hash to {found}, not {named}; {total} bytes read, nothing stored\n"))?;
            answer = answer.with_status(409);
            return Ok(answer);
        }
        let stored = match upload {
            None => {
                let size = buffer.len();
                self.bucket.put(named, buffer).execute().await?;
                format!("single put of {size} bytes")
            }
            Some(open) => {
                if !buffer.is_empty() {
                    let number = u16::try_from(parts.len() + 1).unwrap_or(u16::MAX);
                    parts.push(open.upload_part(number, buffer).await?);
                }
                let count = parts.len();
                open.complete(parts).await?;
                format!("multipart of {count} parts")
            }
        };
        Response::ok(format!(
            "ok put {named}: {total} bytes, {stored}, hash equal, peak buffer {peak} bytes, {} ms\n",
            worker::js_sys::Date::now() - started
        ))
    }
}
