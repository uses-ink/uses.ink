//! `ink deploy list|show|rollback|resume`: a site's deploys, over the edge API.
//!
//! The commands are typed values over [`client::Api`]. Which API, which site and which owner
//! token come from the flags, the environment and the config file, in that order (decision 0061).
//! There is no `ink login` yet, so the token is the owner token of `ink serve --data`.

mod client;
mod failure;
mod view;
mod wire;

use std::future::Future;
use std::time::Duration;

use clap::Args;
use control::Id;

pub(crate) use failure::Failure;

use crate::error::Error;
use crate::exit::Exit;
use crate::output::Io;
use crate::settings::{Environment, Preferences, Process};
use crate::tree::Heading;
use client::{Api, Endpoint, Token};
use view::{Listing, Pointer, Record, Short};
use wire::{Listed, Pointer as Moved, Row, Shown};

/// Which API and which site a remote command talks to.
#[derive(Debug, Args)]
#[command(next_help_heading = Heading::ACCOUNT)]
pub struct Remote {
    /// The API's address, `http://host[:port]` [env: INK_API; config: account.api].
    #[arg(long, value_name = "URL", env = "INK_API")]
    pub api: Option<String>,
    /// The site's id [env: INK_SITE; config: account.site].
    #[arg(long, value_name = "ID", env = "INK_SITE")]
    pub site: Option<String>,
    /// Seconds one call may take.
    #[arg(long, value_name = "SECONDS", env = "INK_TIMEOUT", default_value_t = 30)]
    pub timeout: u64,
}

/// `ink deploy list`.
#[derive(Debug, Args)]
pub struct List {
    /// How many deploys to show, newest first.
    #[arg(long, value_name = "N", default_value_t = 20)]
    pub limit: usize,
    /// Which site.
    #[command(flatten)]
    pub remote: Remote,
}

/// `ink deploy show <DEPLOY>`.
#[derive(Debug, Args)]
pub struct Show {
    /// The deploy: its id, or the first characters of it.
    pub deploy: String,
    /// Which site.
    #[command(flatten)]
    pub remote: Remote,
}

/// `ink deploy rollback <DEPLOY>`.
#[derive(Debug, Args)]
pub struct Rollback {
    /// The deploy to make live: its id, or the first characters of it.
    pub deploy: String,
    /// Which site.
    #[command(flatten)]
    pub remote: Remote,
}

/// A connection ready to make calls: the API and the runtime they run on.
#[derive(Debug)]
struct Calls {
    api: Api,
    runtime: tokio::runtime::Runtime,
}

impl Calls {
    /// Runs `future` to its end.
    fn block<F: Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }
}

impl Remote {
    /// The calls this asks for, with the environment and the config file under the flags.
    fn connect(&self, env: &impl Environment, preferences: &Preferences) -> Result<Calls, Failure> {
        let account = &preferences.account;
        let api = self
            .api
            .clone()
            .or_else(|| account.api.clone())
            .ok_or(Failure::Setting { needed: failure::Needed::Api })?;
        let site = self
            .site
            .clone()
            .or_else(|| account.site.clone())
            .ok_or(Failure::Setting { needed: failure::Needed::Site })?;
        let token = env
            .get("INK_TOKEN")
            .or_else(|| account.token.clone())
            .filter(|token| !token.is_empty())
            .ok_or(Failure::Setting { needed: failure::Needed::Token })?;
        let site = site.parse::<Id<_>>().map_err(|source| Failure::Site { written: site, source })?;
        let api = Api {
            endpoint: Endpoint::parse(&api)?,
            token: Token(token),
            site,
            budget: Duration::from_secs(self.timeout),
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|source| Failure::Runtime { source })?;
        Ok(Calls { api, runtime })
    }

    /// The calls, reading the process environment and the user's config file.
    fn calls(&self) -> Result<Calls, Error> {
        let preferences = Preferences::load(&Process).map_err(|source| Error::Preferences { source })?;
        self.connect(&Process, &preferences).map_err(|source| Error::Deploy { source })
    }
}

/// The deploys of a history, found by what the person typed of an id.
#[derive(Debug)]
struct History<'a>(&'a [Row]);

impl<'a> History<'a> {
    /// The one deploy whose id starts with `written`.
    fn find(&self, written: &str) -> Result<&'a Row, Failure> {
        let mut found = self.0.iter().filter(|row| row.id.to_string().starts_with(written));
        match (found.next(), found.count()) {
            (Some(row), 0) => Ok(row),
            (Some(_), more) => Err(Failure::Ambiguous { written: written.to_owned(), count: more + 1 }),
            (None, _) => Err(Failure::Unknown { written: written.to_owned() }),
        }
    }
}

impl List {
    /// Lists the history, newest first.
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let calls = self.remote.calls()?;
        let all = calls.block(calls.api.deploys()).map_err(|source| Error::Deploy { source })?;
        let rows = all.get(..self.limit.min(all.len())).unwrap_or_default();
        io.line(Listing { rows, total: all.len() })?;
        io.json(&Listed {
            schema: "uses.deploys/v1",
            site: calls.api.site.to_string(),
            total: all.len(),
            deploys: rows,
        })?;
        Ok(Exit::Done)
    }
}

impl Show {
    /// One deploy, with the build that made it and that build's log.
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let calls = self.remote.calls()?;
        let found = calls
            .block(async {
                let all = calls.api.deploys().await?;
                let deploy = History(&all).find(&self.deploy)?.clone();
                let builds = calls.api.builds().await?;
                let build = builds.into_iter().find(|build| build.deploy == Some(deploy.id));
                let log = match &build {
                    Some(build) => calls.api.log(&build.id).await?.unwrap_or_default(),
                    None => Vec::new(),
                };
                Ok::<_, Failure>((deploy, build, log))
            })
            .map_err(|source| Error::Deploy { source })?;
        let (deploy, build, log) = &found;
        io.line(Record { deploy, build: build.as_ref(), log })?;
        io.json(&Shown { schema: "uses.deploy/v1", deploy, build: build.as_ref(), log })?;
        Ok(Exit::Done)
    }
}

impl Rollback {
    /// Makes an earlier deploy the live one; publishing stays paused until `resume`.
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let calls = self.remote.calls()?;
        let moved = calls
            .block(async {
                let all = calls.api.deploys().await?;
                let deploy = History(&all).find(&self.deploy)?;
                calls.api.rollback(deploy.id).await
            })
            .map_err(|source| Error::Deploy { source })?;
        io.line(Pointer(&moved))?;
        io.aside(format_args!(
            "deploys are paused: `ink deploy resume` deploys new pushes again (rolled back to {})\n",
            moved.live.as_ref().map_or_else(String::new, |live| Short(live.deploy).to_string())
        ))?;
        io.json(&Moved { schema: "uses.deploy-pointer/v1", site: calls.api.site.to_string(), moved: &moved })?;
        Ok(Exit::Done)
    }
}

impl Remote {
    /// Lets the site deploy new pushes again.
    pub(crate) fn resume(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let calls = self.calls()?;
        let moved = calls.block(calls.api.resume()).map_err(|source| Error::Deploy { source })?;
        io.line(Pointer(&moved))?;
        io.json(&Moved { schema: "uses.deploy-pointer/v1", site: calls.api.site.to_string(), moved: &moved })?;
        Ok(Exit::Done)
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;

    /// A variable set for one test.
    struct Fixed(BTreeMap<&'static str, &'static str>);

    impl Environment for Fixed {
        fn get(&self, name: &str) -> Option<String> {
            self.0.get(name).map(|value| (*value).to_owned())
        }
    }

    const SITE: &str = "0123456789abcdef0123456789abcdef";
    const FIRST: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const SECOND: &str = "2222222222222222222222222222222222222222222222222222222222222222";

    /// Answers each connection with the next canned `(status, body)`, and gives back the
    /// request lines it was sent.
    fn serve(answers: Vec<(u16, String)>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for (status, body) in answers {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = Vec::new();
                let mut buffer = [0u8; 1024];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    let read = stream.read(&mut buffer).unwrap();
                    request.extend_from_slice(&buffer[..read]);
                }
                let text = String::from_utf8_lossy(&request).into_owned();
                seen.push(format!("{} | {}", text.lines().next().unwrap_or_default(), text.contains("Bearer secret")));
                let reply = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(reply.as_bytes()).unwrap();
            }
            seen
        });
        (address, handle)
    }

    fn remote(api: &str) -> Remote {
        Remote { api: Some(api.to_owned()), site: Some(SITE.to_owned()), timeout: 5 }
    }

    fn env() -> Fixed {
        Fixed(BTreeMap::from([("INK_TOKEN", "secret")]))
    }

    fn history() -> String {
        format!(
            r#"{{"items":[{{"id":"{FIRST}","created":"2026-11-27T14:03:00Z","build":null,"files":61,"bytes":1200,"live":true}},{{"id":"{SECOND}","created":"2026-11-25T18:12:00Z","build":null,"files":58,"bytes":900,"live":false}}]}}"#
        )
    }

    #[test]
    fn a_history_is_read_with_the_token_as_a_bearer() {
        let (api, server) = serve(vec![(200, history())]);
        let calls = remote(&api).connect(&env(), &Preferences::default()).unwrap();
        let rows = calls.block(calls.api.deploys()).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows[0].live);
        assert_eq!(server.join().unwrap(), [format!("GET /v1/sites/{SITE}/deploys HTTP/1.1 | true")]);
    }

    #[test]
    fn a_rollback_names_the_deploy_by_the_start_of_its_id() {
        let moved = format!(
            r#"{{"live":{{"deploy":"{SECOND}","previous":"{FIRST}","paused":true,"since":"2026-11-27T15:00:00Z"}},"was_paused":null}}"#
        );
        let (api, server) = serve(vec![(200, history()), (200, moved)]);
        let calls = remote(&api).connect(&env(), &Preferences::default()).unwrap();
        let moved = calls
            .block(async {
                let all = calls.api.deploys().await?;
                let target = History(&all).find("22")?;
                calls.api.rollback(target.id).await
            })
            .unwrap();
        assert_eq!(moved.live.map(|live| live.paused), Some(true));
        assert_eq!(
            server.join().unwrap().last().cloned(),
            Some(format!("POST /v1/sites/{SITE}/rollback HTTP/1.1 | true"))
        );
    }

    #[test]
    fn what_the_api_refuses_has_its_own_exit_code() {
        let said = |status: u16| (status, r#"{"problem":"x","message":"no","causes":[]}"#.to_owned());
        for (status, exit) in [(401, Exit::Auth), (404, Exit::Usage), (409, Exit::Conflict), (503, Exit::Network)] {
            let (api, server) = serve(vec![said(status)]);
            let calls = remote(&api).connect(&env(), &Preferences::default()).unwrap();
            let failure = calls.block(calls.api.resume()).unwrap_err();
            assert_eq!(failure.exit(), exit, "{status}");
            drop(server);
        }
    }

    #[test]
    fn a_missing_setting_names_where_to_give_it() {
        let none = Fixed(BTreeMap::new());
        let bare = Remote { api: None, site: None, timeout: 5 };
        let missing = |remote: &Remote, env: &Fixed| remote.connect(env, &Preferences::default()).unwrap_err();
        assert_eq!(missing(&bare, &none).exit(), Exit::Usage);
        let no_token = missing(&remote("http://127.0.0.1:1"), &none);
        assert_eq!((no_token.exit(), no_token.kind()), (Exit::Auth, "deploy.setting.missing"));
        assert!(no_token.to_string().contains("INK_TOKEN"));
    }

    #[test]
    fn the_config_file_fills_what_the_flags_leave_out() {
        let preferences: Preferences =
            toml::from_str(&format!("[account]\napi = \"http://127.0.0.1:1\"\nsite = \"{SITE}\"\ntoken = \"t\"\n"))
                .unwrap();
        let bare = Remote { api: None, site: None, timeout: 5 };
        assert!(bare.connect(&Fixed(BTreeMap::new()), &preferences).is_ok());
    }

    #[test]
    fn an_id_is_found_by_its_start_and_only_when_that_is_one_deploy() {
        let rows: Vec<Row> = serde_json::from_str::<wire::Items<Row>>(&history()).unwrap().items;
        let history = History(&rows);
        assert_eq!(history.find("1").unwrap().files, 61);
        assert!(matches!(history.find("3"), Err(Failure::Unknown { .. })));
        let both = format!("{FIRST}");
        assert!(history.find(&both).is_ok());
    }
}
