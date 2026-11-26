//! M0 spike 3: a Durable Object per build starts a Container, streams the container's output to
//! the HTTP client as it arrives, and writes the collected log to R2.
//!
//! Routes (worker):
//! - `POST /build/<name>/run?lines=&interval=&internet=` stream over the container's HTTP port
//! - `POST /build/<name>/exec?lines=&interval=` stream real stdout of an `exec`ed process
//! - `POST /build/<name>/egress?host=` ask the container to fetch `http://<host>/`
//! - `POST /build/<name>/stop` destroy the container
//! - `GET /log/<key>` read a log back from R2
//!
//! Egress: the container starts with `enableInternet: false`, and the DO registers outbound
//! intercepts through `js_sys::Reflect`, because workers-rs 0.9 binds neither
//! `interceptOutboundHttp` nor `ctx.exports` (#1072). The handler is this worker's own default
//! entrypoint with `props`; `fetch` tells an intercepted request from a public one by its props,
//! which only code holding `ctx.exports` can set.

use std::collections::HashMap;
use std::time::Duration;

use futures_channel::mpsc;
use futures_util::{Stream, StreamExt};
use serde::Deserialize;
use worker::js_sys::{Array, Function, Object, Promise, Reflect};
use worker::wasm_bindgen::{JsCast, JsValue};
use worker::wasm_bindgen_futures::{JsFuture, spawn_local};
use worker::*;

/// Hosts the container may reach. `allowed.test` does not exist; the worker answers for it.
const ALLOWED: &[&str] = &["allowed.test", "example.com"];
/// Intercepted to show a deny: registered, then refused by the handler.
const DENIED: &[&str] = &["denied.test"];

#[derive(Deserialize)]
struct EgressProps {
    egress: bool,
    build: String,
}

#[event(fetch)]
async fn fetch(req: Request, env: Env, ctx: Context) -> Result<Response> {
    // Only a loopback entrypoint created by the DO carries these props.
    if let Ok(props) = ctx.props::<EgressProps>()
        && props.egress
    {
        return egress(req, &props).await;
    }
    let url = req.url()?;
    let segments: Vec<String> = url
        .path_segments()
        .map(|s| s.map(str::to_owned).collect())
        .unwrap_or_default();
    let segments: Vec<&str> = segments.iter().map(String::as_str).collect();
    match (req.method(), segments.as_slice()) {
        (Method::Post, ["build", name, action]) => {
            let stub = env.durable_object("BUILD")?.id_from_name(name)?.get_stub()?;
            let mut target = Url::parse("http://build/")?;
            target.set_path(action);
            let mut query: Vec<(String, String)> = url.query_pairs().into_owned().collect();
            query.push(("name".into(), (*name).into()));
            target.query_pairs_mut().extend_pairs(query);
            stub.fetch_with_str(target.as_str()).await
        }
        (Method::Get, ["log", rest @ ..]) => {
            let key = rest.join("/");
            match env.bucket("LOGS")?.get(&key).execute().await? {
                Some(object) => match object.body() {
                    Some(body) => Response::from_bytes(body.bytes().await?),
                    None => Response::error("object has no body", 500),
                },
                None => Response::error(format!("no object at {key}"), 404),
            }
        }
        _ => Response::error("not found", 404),
    }
}

/// The outbound handler: every intercepted request from a container lands here.
async fn egress(req: Request, props: &EgressProps) -> Result<Response> {
    let url = req.url()?;
    let host = url.host_str().unwrap_or_default().to_owned();
    console_log!("egress build={} {} {}", props.build, req.method(), url);
    if host == "allowed.test" {
        return Response::ok(format!(
            "allowed.test answered by the worker for build {}\n",
            props.build
        ));
    }
    if ALLOWED.contains(&host.as_str()) {
        return Fetch::Request(req).send().await;
    }
    Response::error(format!("egress to {host} denied for build {}\n", props.build), 403)
}

#[durable_object]
pub struct Build {
    state: State,
    env: Env,
}

impl DurableObject for Build {
    fn new(state: State, env: Env) -> Self {
        Self { state, env }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        let url = req.url()?;
        let q: HashMap<String, String> = url.query_pairs().into_owned().collect();
        let name = q.get("name").cloned().unwrap_or_else(|| "unnamed".into());
        let lines = q.get("lines").cloned().unwrap_or_else(|| "10".into());
        let interval = q.get("interval").cloned().unwrap_or_else(|| "200".into());
        let internet = q.get("internet").is_some_and(|v| v == "on");
        let intercept = !q.get("intercept").is_some_and(|v| v == "off");
        match url.path() {
            "/run" => {
                let notes = self.ensure_started(&name, internet, intercept).await?;
                let port = self.container()?.get_tcp_port(8080)?;
                let mut resp = port
                    .fetch(format!("http://container/run?lines={lines}&interval={interval}"), None)
                    .await?;
                let body = resp.stream()?;
                self.pipe(&name, "run", notes, body)
            }
            "/exec" => {
                let notes = self.ensure_started(&name, internet, intercept).await?;
                let stdout = self.exec(&["/emit", "lines", &lines, &interval]).await?;
                self.pipe(&name, "exec", notes, stdout)
            }
            "/egress" => {
                let mut notes = self.ensure_started(&name, internet, intercept).await?;
                let host = q.get("host").cloned().unwrap_or_else(|| "example.com".into());
                let port = self.container()?.get_tcp_port(8080)?;
                let mut resp = port
                    .fetch(format!("http://container/egress?host={host}"), None)
                    .await?;
                notes.push(resp.text().await?);
                Response::ok(notes.join("\n"))
            }
            // No container: an in-DO producer stands in, to test DO streaming and R2 alone.
            "/local" => {
                let count: u32 = lines.parse().unwrap_or(10);
                let every: u64 = interval.parse().unwrap_or(200);
                let source = futures_util::stream::unfold(0u32, move |i| async move {
                    if i >= count {
                        return None;
                    }
                    if i > 0 {
                        Delay::from(Duration::from_millis(every)).await;
                    }
                    let line = format!("line {i} t={}\n", Date::now().as_millis());
                    Some((Ok(line.into_bytes()), i + 1))
                });
                self.pipe(&name, "local", vec!["source: in-DO producer, no container".into()], source)
            }
            // Calls the egress handler the way the container sidecar would, without a container.
            "/handler" => {
                let host = q.get("host").cloned().unwrap_or_else(|| "denied.test".into());
                let handler: Fetcher = self.egress_handler(&name)?.unchecked_into::<worker_sys::Fetcher>().into();
                let mut resp = handler.fetch(format!("http://{host}/"), None).await?;
                let status = resp.status_code();
                let body = resp.text().await?;
                Response::ok(format!("handler for {host}: {status} {body}"))
            }
            "/stop" => {
                let container = self.container()?;
                if container.running() {
                    container.destroy(None).await?;
                    Response::ok("destroyed\n")
                } else {
                    Response::ok("not running\n")
                }
            }
            other => Response::error(format!("no DO route {other}"), 404),
        }
    }
}

impl Build {
    fn container(&self) -> Result<Container> {
        self.state
            .container()
            .ok_or_else(|| Error::RustError("no container on this Durable Object".into()))
    }

    /// Starts the container if needed, returning what happened as lines for the client.
    async fn ensure_started(&self, name: &str, internet: bool, intercept: bool) -> Result<Vec<String>> {
        let container = self.container()?;
        if container.running() {
            return Ok(vec!["container already running".into()]);
        }
        let mut notes = Vec::new();
        let t0 = Date::now().as_millis();
        console_log!("ensure_started: intercept={intercept} internet={internet}");
        match if intercept { self.intercept(&container, name).await } else { Ok(0) } {
            Ok(n) => notes.push(format!(
                "registered {n} outbound intercepts in {} ms",
                Date::now().as_millis() - t0
            )),
            Err(e) => notes.push(format!("intercepts failed: {e}")),
        }
        let mut options = ContainerStartupOptions::new();
        options.enable_internet(internet);
        let t1 = Date::now().as_millis();
        container.start(Some(options))?;
        console_log!("ensure_started: start() returned");
        let port = container.get_tcp_port(8080)?;
        let mut tries = 0u32;
        loop {
            tries += 1;
            match port.fetch("http://container/ping", None).await {
                Ok(r) if r.status_code() == 200 => break,
                Ok(r) if tries > 400 => {
                    return Err(Error::RustError(format!("ping answered {}", r.status_code())));
                }
                Err(e) if tries > 400 => return Err(e),
                _ => Delay::from(Duration::from_millis(25)).await,
            }
        }
        notes.push(format!(
            "container started (internet {}) and answered /ping in {} ms after {tries} tries",
            if internet { "on" } else { "off" },
            Date::now().as_millis() - t1
        ));
        Ok(notes)
    }

    /// `ctx.exports.default({ props: { egress: true, build } })`: a loopback to this worker's own
    /// `fetch`, carrying props a public request cannot have.
    fn egress_handler(&self, name: &str) -> Result<JsValue> {
        let exports = Reflect::get(self.state.as_raw(), &"exports".into())?;
        if exports.is_undefined() {
            return Err(Error::RustError(
                "ctx.exports is undefined: compatibility date before 2025-11-17?".into(),
            ));
        }
        let default: Function = Reflect::get(&exports, &"default".into())?
            .dyn_into()
            .map_err(|v| Error::RustError(format!("ctx.exports.default is not callable: {v:?}")))?;
        let props = Object::new();
        Reflect::set(&props, &"egress".into(), &JsValue::TRUE)?;
        Reflect::set(&props, &"build".into(), &name.into())?;
        let options = Object::new();
        Reflect::set(&options, &"props".into(), &props)?;
        // Not `default.call1(..)`: wasm-bindgen emits `default.call(..)`, and on an RPC stub
        // `.call` is a remote method name, so it tries to serialize the stub (DataCloneError).
        Ok(Reflect::apply(&default, &exports, &Array::of1(&options))?)
    }

    /// Registers intercepts through raw JS; workers-rs 0.9 binds no `interceptOutboundHttp`.
    async fn intercept(&self, container: &Container, name: &str) -> Result<usize> {
        let handler = self.egress_handler(name)?;
        console_log!("intercept: handler created");
        let raw: &JsValue = container.as_ref();
        let http: Function = Reflect::get(raw, &"interceptOutboundHttp".into())?
            .dyn_into()
            .map_err(|_| Error::RustError("container.interceptOutboundHttp missing".into()))?;
        let mut n = 0;
        for host in ALLOWED.iter().chain(DENIED) {
            let p: Promise = http.call2(raw, &(*host).into(), &handler)?.dyn_into()?;
            JsFuture::from(p).await?;
            console_log!("intercept: {host} registered");
            n += 1;
        }
        Ok(n)
    }

    /// `container.exec(cmd, { stdout: "pipe", stderr: "combined" })`, unbound in workers-rs 0.9.
    async fn exec(&self, cmd: &[&str]) -> Result<ByteStream> {
        let container = self.container()?;
        let raw: &JsValue = container.as_ref();
        let exec: Function = Reflect::get(raw, &"exec".into())?
            .dyn_into()
            .map_err(|_| Error::RustError("container.exec missing in this runtime".into()))?;
        let args: Array = cmd.iter().map(|s| JsValue::from_str(s)).collect();
        let options = Object::new();
        Reflect::set(&options, &"stdout".into(), &"pipe".into())?;
        Reflect::set(&options, &"stderr".into(), &"combined".into())?;
        let p: Promise = exec.call2(raw, &args, &options)?.dyn_into()?;
        let process = JsFuture::from(p).await?;
        let stdout: web_sys::ReadableStream = Reflect::get(&process, &"stdout".into())?
            .dyn_into()
            .map_err(|_| Error::RustError("exec process has no stdout stream".into()))?;
        Ok(ByteStream::from(stdout))
    }

    /// Forwards each chunk to the client as it arrives, keeps a copy, writes it to R2 at the end.
    fn pipe(
        &self,
        name: &str,
        kind: &str,
        notes: Vec<String>,
        source: impl Stream<Item = Result<Vec<u8>>> + 'static,
    ) -> Result<Response> {
        let bucket = self.env.bucket("LOGS")?;
        let key = format!("log/{name}/{kind}-{}.log", Date::now().as_millis());
        let (tx, rx) = mpsc::unbounded::<Vec<u8>>();
        // A closed receiver means the client left; the log still goes to R2, so sends are not checked.
        let say = move |tx: &mpsc::UnboundedSender<Vec<u8>>, line: String| {
            let _ = tx.unbounded_send(format!("# {line}\n").into_bytes());
        };
        for n in notes {
            say(&tx, n);
        }
        spawn_local(async move {
            let mut source = Box::pin(source);
            let mut log = Vec::new();
            let mut chunks = 0u32;
            while let Some(chunk) = source.next().await {
                match chunk {
                    Ok(c) => {
                        chunks += 1;
                        log.extend_from_slice(&c);
                        let _ = tx.unbounded_send(c);
                    }
                    Err(e) => {
                        say(&tx, format!("source stream failed: {e}"));
                        break;
                    }
                }
            }
            let bytes = log.len();
            let t = Date::now().as_millis();
            match bucket.put(&key, log).execute().await {
                Ok(_) => say(
                    &tx,
                    format!(
                        "r2 put {key}: {bytes} bytes, {chunks} chunks, {} ms",
                        Date::now().as_millis() - t
                    ),
                ),
                Err(e) => say(&tx, format!("r2 put {key} failed: {e}")),
            }
        });
        let mut resp = Response::from_stream(rx.map(Ok::<Vec<u8>, Error>))?;
        resp.headers_mut().set("content-type", "text/plain; charset=utf-8")?;
        Ok(resp)
    }
}
