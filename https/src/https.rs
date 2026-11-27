//! The settings of an https connection and the options they make for `gix`.

use std::any::Any;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gix::protocol::transport::client::blocking_io::http::{self, reqwest::Options};
use rustls::RootCertStore;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::pem::PemObject;

use crate::{Follow, Misconfigured};

/// The settings of an https connection: the roots it trusts, its timeouts and its redirects.
#[derive(Clone, Debug)]
pub struct Https {
    /// How long to wait for a connection to be made.
    pub connect: Duration,
    /// How long one request may take in all, its body read included. A clone is one request for
    /// its pack, so this is also the longest a pack may take to arrive.
    pub total: Duration,
    /// Which redirects are followed.
    pub follow: Follow,
    /// Certificate authorities trusted beside the bundled roots.
    pub roots: Vec<CertificateDer<'static>>,
}

impl Default for Https {
    /// 10 seconds to connect, 10 minutes for a request, the first request's redirects, and the
    /// bundled roots alone.
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(10),
            total: Duration::from_secs(600),
            follow: Follow::First,
            roots: Vec::new(),
        }
    }
}

impl Https {
    /// Also trusts `root`, a certificate authority in DER form, beside the bundled roots: a
    /// forge with a private authority, or a test. It adds trust for the chain it signs and turns
    /// no check off.
    #[must_use]
    pub fn trusting(mut self, root: CertificateDer<'static>) -> Self {
        self.roots.push(root);
        self
    }

    /// Also trusts every certificate of the PEM file at `path`, as [`Https::trusting`] does.
    ///
    /// # Errors
    ///
    /// [`Misconfigured::Roots`] when the file cannot be read or holds a malformed certificate.
    pub fn trusting_file(mut self, path: &Path) -> Result<Self, Misconfigured> {
        let roots = |source| Misconfigured::Roots { path: path.to_path_buf(), source };
        for root in CertificateDer::pem_file_iter(path).map_err(roots)? {
            self.roots.push(root.map_err(roots)?);
        }
        Ok(self)
    }

    /// The TLS client: ring, the safe protocol versions, the bundled roots and the extra ones,
    /// HTTP/1.1 only.
    fn client(&self) -> Result<rustls::ClientConfig, Misconfigured> {
        let mut roots = RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        for root in &self.roots {
            roots.add(root.clone()).map_err(Misconfigured::Tls)?;
        }
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut config = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(Misconfigured::Tls)?
            .with_root_certificates(roots)
            .with_no_client_auth();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        Ok(config)
    }

    /// The transport options of a connection to an https url, to give to the connection with
    /// `set_transport_options` before it lists or fetches. They are `gix`'s defaults with no
    /// git configuration read: every setting of the transport is the one made here.
    ///
    /// # Errors
    ///
    /// [`Misconfigured`] when the TLS client cannot be set up.
    pub fn options(&self) -> Result<Box<dyn Any>, Misconfigured> {
        let client = self.client()?;
        let (connect, total) = (self.connect, self.total);
        let options = http::Options {
            follow_redirects: self.follow.into(),
            ssl_verify: true,
            connect_timeout: Some(connect),
            user_agent: Some(gix::env::agent().into()),
            ..http::Options::default()
        };
        let backend = Options {
            configure_client: Some(Box::new(move |builder| {
                Ok(builder
                    .tls_backend_preconfigured(client.clone())
                    .https_only(true)
                    .http1_only()
                    .no_proxy()
                    .connect_timeout(connect)
                    .timeout(total))
            })),
            configure_request: None,
        };
        Ok(Box::new(http::Options { backend: Some(Arc::new(Mutex::new(backend))), ..options }))
    }
}
