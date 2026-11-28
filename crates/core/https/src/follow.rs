//! Which redirects a request follows.

use gix::protocol::transport::client::blocking_io::http::options::FollowRedirects;

/// Which redirects a request follows. A redirect that leaves https is never followed, whatever
/// this says: `gix` refuses a scheme change other than http to https, and the client allows https
/// only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Follow {
    /// None: a redirect is the end of the request.
    Never,
    /// Those of the first request of a connection, as `git` does by default.
    #[default]
    First,
    /// Those of every request. `gix` stops a chain after 50 redirects; that number is not a
    /// setting.
    Always,
}

impl From<Follow> for FollowRedirects {
    fn from(follow: Follow) -> Self {
        match follow {
            Follow::Never => Self::None,
            Follow::First => Self::Initial,
            Follow::Always => Self::All,
        }
    }
}
