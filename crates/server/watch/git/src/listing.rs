//! One connection to a remote, as `git ls-remote` makes it.

use std::path::PathBuf;

use gix::remote::Direction;
use https::Https;
use secret::Plain;

use crate::Unlisted;
use crate::tips::Tips;

/// The user and the password or token a connection answers a server's request for credentials
/// with.
#[derive(Debug)]
pub(crate) struct Login {
    pub(crate) user: String,
    pub(crate) password: Plain,
}

/// What to list, and where to run the connection.
#[derive(Debug)]
pub(crate) struct Listing {
    pub(crate) scratch: PathBuf,
    pub(crate) url: String,
    pub(crate) revision: Option<String>,
    pub(crate) login: Option<Login>,
    /// The https settings, for an https url.
    pub(crate) https: Option<Https>,
}

impl Listing {
    /// The refspecs that ask for every branch and tag, and for `HEAD`, so a server that filters by
    /// prefix sends all three. Nothing is fetched, so where they would land is never written.
    const SPECS: [&'static str; 3] = ["+refs/heads/*:refs/heads/*", "+refs/tags/*:refs/tags/*", "+HEAD:refs/ink/head"];

    /// The empty repository connections run in, made when it is missing.
    fn repository(&self) -> Result<gix::Repository, Unlisted> {
        if self.scratch.join("HEAD").is_file() {
            return gix::open_opts(&self.scratch, gix::open::Options::isolated()).map_err(Unlisted::remote);
        }
        let scratch = |source| Unlisted::Scratch { path: self.scratch.clone(), source };
        std::fs::create_dir_all(&self.scratch).map_err(scratch)?;
        gix::init_bare(&self.scratch).map_err(Unlisted::remote)
    }

    /// The credential callback: every request is answered with the login, and nothing is stored.
    fn answer(
        login: Login,
    ) -> impl FnMut(gix::credentials::helper::Action) -> gix::Result<Option<gix::credentials::protocol::Outcome>> {
        move |action| {
            Ok(match action {
                gix::credentials::helper::Action::Get(context) => Some(gix::credentials::protocol::Outcome {
                    credentials: gix::sec::identity::Credentials::UsernamePassword {
                        username: login.user.clone(),
                        password: String::from_utf8_lossy(login.password.expose()).into_owned(),
                        oauth_refresh_token: None,
                    },
                    next: context.into(),
                }),
                _ => None,
            })
        }
    }

    /// Lists the remote's references and picks the commit the revision names. Blocking.
    pub(crate) fn run(self) -> Result<watch::Head, Unlisted> {
        let repo = self.repository()?;
        let url = gix::url::parse(gix::bstr::BStr::new(&self.url)).map_err(Unlisted::remote)?;
        let remote = repo.remote_at(url).map_err(Unlisted::remote)?;
        let remote = remote.with_refspecs(Self::SPECS, Direction::Fetch).map_err(Unlisted::remote)?;
        let mut connection = remote.connect(Direction::Fetch).map_err(Unlisted::remote)?;
        if let Some(https) = &self.https {
            connection.set_transport_options(https.options().map_err(Unlisted::Https)?);
        }
        if let Some(login) = self.login {
            connection.set_credentials(Self::answer(login));
        }
        let asked = connection.ref_map(gix::progress::Discard, gix::remote::ref_map::Options::default());
        let (map, _) = asked.map_err(|source| Unlisted::Remote { source })?;
        let commit = Tips(&map.remote_refs).head(self.revision.as_deref()).map_err(Unlisted::Missing)?;
        commit.to_hex().to_string().parse().map_err(Unlisted::Commit)
    }
}
