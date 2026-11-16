use anyhow::{anyhow, bail, Context, Result};
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_rustls::rustls::pki_types::pem::PemObject;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use tokio_rustls::rustls::{
    self, crypto::CryptoProvider, ClientConfig, RootCertStore, ServerConfig,
};
use tokio_rustls::server::TlsStream;
use tokio_rustls::{TlsAcceptor, TlsConnector};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct TlsFiles {
    pub cert: PathBuf,
    pub key: PathBuf,
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

pub(crate) fn acceptor(files: &TlsFiles) -> Result<TlsAcceptor> {
    let certs = CertificateDer::pem_file_iter(&files.cert)
        .and_then(|certs| certs.collect::<Result<Vec<_>, _>>())
        .with_context(|| format!("cannot read the certificate {}", files.cert.display()))?;
    if certs.is_empty() {
        bail!("no certificate in {}", files.cert.display());
    }
    let key = PrivateKeyDer::from_pem_file(&files.key)
        .with_context(|| format!("cannot read the private key {}", files.key.display()))?;
    let config = ServerConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("the certificate and the key do not make a TLS identity")?;
    Ok(TlsAcceptor::from(Arc::new(config)))
}

pub(crate) struct TlsListener {
    handshaken: mpsc::Receiver<(TlsStream<TcpStream>, SocketAddr)>,
    addr: SocketAddr,
}

impl TlsListener {
    pub(crate) fn new(tcp: TcpListener, acceptor: TlsAcceptor) -> io::Result<Self> {
        let addr = tcp.local_addr()?;
        let (tx, handshaken) = mpsc::channel(64);
        tokio::spawn(async move {
            while !tx.is_closed() {
                let (socket, peer) = match tcp.accept().await {
                    Ok(pair) => pair,
                    Err(e) => {
                        tracing::warn!("hub accept failed: {e}");
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    }
                };
                let (acceptor, tx) = (acceptor.clone(), tx.clone());
                tokio::spawn(async move {
                    match tokio::time::timeout(HANDSHAKE_TIMEOUT, acceptor.accept(socket)).await {
                        Ok(Ok(stream)) => {
                            let _ = tx.send((stream, peer)).await;
                        }
                        Ok(Err(e)) => tracing::debug!(%peer, "TLS handshake failed: {e}"),
                        Err(_) => tracing::debug!(%peer, "TLS handshake timed out"),
                    }
                });
            }
        });
        Ok(Self { handshaken, addr })
    }
}

impl axum::serve::Listener for TlsListener {
    type Io = TlsStream<TcpStream>;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        match self.handshaken.recv().await {
            Some(pair) => pair,
            None => std::future::pending().await,
        }
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        Ok(self.addr)
    }
}

pub(crate) trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

fn system_roots() -> &'static RootCertStore {
    static ROOTS: OnceLock<RootCertStore> = OnceLock::new();
    ROOTS.get_or_init(|| {
        let mut roots = RootCertStore::empty();
        let found = rustls_native_certs::load_native_certs();
        for e in found.errors {
            tracing::debug!("skipped a system certificate: {e}");
        }
        roots.add_parsable_certificates(found.certs);
        roots
    })
}

fn connector(extra_ca: Option<&Path>) -> Result<TlsConnector> {
    let mut roots = system_roots().clone();
    if let Some(path) = extra_ca {
        let certs = CertificateDer::pem_file_iter(path)
            .and_then(|certs| certs.collect::<Result<Vec<_>, _>>())
            .with_context(|| format!("cannot read the CA bundle {}", path.display()))?;
        if certs.is_empty() {
            bail!("no certificate in the CA bundle {}", path.display());
        }
        for cert in certs {
            roots
                .add(cert)
                .with_context(|| format!("unusable certificate in {}", path.display()))?;
        }
    }
    let config = ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(TlsConnector::from(Arc::new(config)))
}

#[derive(Clone)]
pub(crate) struct Endpoint {
    pub(crate) authority: String,
    host: String,
    pub(crate) tls: bool,
    connector: Option<TlsConnector>,
}

impl Endpoint {
    pub(crate) fn parse(url: &str, extra_ca: Option<&Path>) -> Result<Self> {
        let (tls, rest) = if let Some(rest) = url.strip_prefix("https://") {
            (true, rest)
        } else if let Some(rest) = url.strip_prefix("http://") {
            (false, rest)
        } else {
            bail!("the hub url must start with http:// or https://");
        };
        let rest = rest.trim_end_matches('/');
        let has_port = rest
            .rsplit_once(':')
            .is_some_and(|(_, port)| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()));
        let default_port = if tls { 443 } else { 80 };
        let authority = if has_port {
            rest.to_string()
        } else {
            format!("{rest}:{default_port}")
        };
        let host = authority
            .rsplit_once(':')
            .map(|(host, _)| host.trim_start_matches('[').trim_end_matches(']'))
            .unwrap_or_default()
            .to_string();
        if host.is_empty() {
            bail!("the hub url names no host");
        }
        let connector = if tls {
            Some(connector(extra_ca)?)
        } else {
            None
        };
        Ok(Self {
            authority,
            host,
            tls,
            connector,
        })
    }

    pub(crate) async fn connect(&self) -> Result<Box<dyn Io>> {
        let tcp = TcpStream::connect(&self.authority)
            .await
            .with_context(|| format!("cannot reach the hub at {}", self.authority))?;
        let Some(connector) = &self.connector else {
            return Ok(Box::new(tcp));
        };
        let name = ServerName::try_from(self.host.clone())
            .map_err(|_| anyhow!("{} is not a name a TLS certificate can carry", self.host))?;
        let stream = connector
            .connect(name, tcp)
            .await
            .with_context(|| format!("TLS with the hub at {} failed", self.authority))?;
        Ok(Box::new(stream))
    }
}
