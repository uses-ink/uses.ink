//! The preview and the server `ink serve` runs.

use std::fmt;

/// The preview or server could not be set up or kept running.
#[derive(Debug)]
#[non_exhaustive]
pub enum ServeError {
    /// The preview server could not listen or serve.
    Serve {
        /// What the system said.
        source: std::io::Error,
    },
    /// `ink serve` could not start its runtime or wait for Ctrl-C.
    Run {
        /// What the system said.
        source: std::io::Error,
    },
    /// The folder `ink serve` was given is no deploy it can serve.
    Deploys {
        /// Why.
        source: edge_folder::Error,
    },
    /// `ink serve` could not listen on its address.
    Listen {
        /// The address, as given.
        address: std::net::SocketAddr,
        /// What the system said.
        source: std::io::Error,
    },
    /// The edge could not be built.
    Edge {
        /// Why.
        source: edge::Error,
    },
    /// The server could not be built or run.
    Server {
        /// Why.
        source: edge_axum::Error,
    },
    /// The control plane `ink serve --data` runs could not be set up.
    Product {
        /// Why.
        source: crate::selfhost::product::Unready,
    },
}

impl ServeError {
    /// What kind of error it is, as `--json` names it.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Serve { .. } => "preview.serve.failed",
            Self::Run { .. } => "serve.run.failed",
            Self::Deploys { .. } => "serve.folder.unservable",
            Self::Listen { .. } => "serve.address.unusable",
            Self::Edge { .. } => "serve.edge.invalid",
            Self::Server { .. } => "serve.server.invalid",
            Self::Product { .. } => "serve.data.unusable",
        }
    }
}

impl fmt::Display for ServeError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serve { .. } => write!(out, "the preview could not be served"),
            Self::Run { .. } => write!(out, "`ink serve` could not keep running"),
            Self::Deploys { .. } => write!(out, "the folder is not a deploy `ink serve` can serve"),
            Self::Listen { address, .. } => write!(out, "`ink serve` could not listen on {address}"),
            Self::Edge { .. } => write!(out, "the edge could not be built"),
            Self::Server { .. } => write!(out, "the server could not be started"),
            Self::Product { .. } => write!(out, "the control plane could not be set up"),
        }
    }
}

impl std::error::Error for ServeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Serve { source } | Self::Run { source } | Self::Listen { source, .. } => Some(source),
            Self::Deploys { source } => Some(source),
            Self::Edge { source } => Some(source),
            Self::Server { source } => Some(source),
            Self::Product { source } => Some(source),
        }
    }
}
