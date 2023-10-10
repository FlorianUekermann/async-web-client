mod http;
mod ws;

use std::{
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

pub use crate::http::*;
use async_net::TcpStream;
use futures::{AsyncRead, AsyncWrite};
use futures_rustls::{
    client::TlsStream,
    rustls::{client::InvalidDnsNameError, ClientConfig, OwnedTrustAnchor, RootCertStore, ServerName},
    TlsConnector,
};
pub use ws::*;

pub enum Transport {
    Tcp(TcpStream),
    Tls(TlsStream<TcpStream>),
}

impl Transport {
    async fn connect(tls: Option<Arc<ClientConfig>>, host: &str, port: u16) -> Result<Self, TransportError> {
        let server = ServerName::try_from(host).map_err(|err| TransportError::InvalidDnsName(Arc::new(err)))?;
        let tcp = match &server {
            ServerName::DnsName(name) => TcpStream::connect((name.as_ref(), port)).await,
            ServerName::IpAddress(ip) => TcpStream::connect((ip.clone(), port)).await,
            _ => unreachable!(),
        }
        .map_err(|err| TransportError::TcpConnect(Arc::new(err)))?;
        let transport = match tls {
            None => Transport::Tcp(tcp),
            Some(client_config) => {
                let tls = TlsConnector::from(client_config)
                    .connect(server, tcp)
                    .await
                    .map_err(|err| TransportError::TlsConnect(Arc::new(err)))?;
                Transport::Tls(tls)
            }
        };
        Ok(transport)
    }
}

impl Unpin for Transport {}

impl AsyncRead for Transport {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Transport::Tcp(tcp) => Pin::new(tcp).poll_read(cx, buf),
            Transport::Tls(tls) => Pin::new(tls).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Transport {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Transport::Tcp(tcp) => Pin::new(tcp).poll_write(cx, buf),
            Transport::Tls(tls) => Pin::new(tls).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Transport::Tcp(tcp) => Pin::new(tcp).poll_flush(cx),
            Transport::Tls(tls) => Pin::new(tls).poll_flush(cx),
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Transport::Tcp(tcp) => Pin::new(tcp).poll_close(cx),
            Transport::Tls(tls) => Pin::new(tls).poll_close(cx),
        }
    }
}

use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum TransportError {
    #[error("invalid host name: {0:?}")]
    InvalidDnsName(Arc<InvalidDnsNameError>),
    #[error("tcp connect error: {0:?}")]
    TcpConnect(Arc<io::Error>),
    #[error("tls connect error: {0:?}")]
    TlsConnect(Arc<io::Error>),
}

lazy_static::lazy_static! {
    pub (crate) static ref DEFAULT_CLIENT_CONFIG: Arc<ClientConfig> = {
        let roots = webpki_roots::TLS_SERVER_ROOTS
        .iter()
        .map(|t| {OwnedTrustAnchor::from_subject_spki_name_constraints(t.subject,t.spki,t.name_constraints)});
        let mut root_store = RootCertStore::empty();
        root_store.add_trust_anchors(roots);
        let config = ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        Arc::new(config)
    };
}
