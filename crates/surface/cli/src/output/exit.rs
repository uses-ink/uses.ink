//! The exit codes `ink` uses, from research/flow.md section 7.

/// How a command ended. Codes 3 to 5 (sign-in, network, conflicts) belong to commands that talk
/// to an API: `ink deploy` today.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Exit {
    /// `0`: done; warnings, if any, were printed.
    Done,
    /// `1`: the build or check failed on diagnostics. `ink build` wrote nothing: the previous
    /// build in the output folder is left as it was.
    Content,
    /// `2`: a usage error, a site's folder that is missing or cannot be listed, or an output
    /// folder `ink build` will not clear or write into; a refused output folder is left as it was.
    Usage,
    /// `3`: not signed in, or the API refused the token.
    Auth,
    /// `4`: the network or the service was unavailable. Safe to retry.
    Network,
    /// `5`: the API refused the change as the site stands, such as a deploy that is live.
    Conflict,
    /// `6`: `--strict` was given and there were warnings. As with `1`, `ink build` wrote nothing.
    Strict,
}

impl Exit {
    /// The process exit code.
    pub const fn code(self) -> u8 {
        match self {
            Self::Done => 0,
            Self::Content => 1,
            Self::Usage => 2,
            Self::Auth => 3,
            Self::Network => 4,
            Self::Conflict => 5,
            Self::Strict => 6,
        }
    }
}
