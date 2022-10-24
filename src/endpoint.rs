use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::task::{Context, Poll};
use futures_util::StreamExt;
use crate::connection::{QPConnection, QPNewConnection};
use crate::ServerConfig;

pub struct QPIncoming(quinn::Incoming);

pub struct QPConnecting(quinn::Connecting);

impl Future for QPConnecting {
    type Output = Result<QPNewConnection, quinn::ConnectionError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        return match Pin::new(&mut self.0).poll(cx) {
            Poll::Ready(Ok(value)) => {
                let connection = QPConnection(value.connection.clone());
                Poll::Ready(Ok(QPNewConnection::new(value, connection)))
            }
            Poll::Ready(Err(value)) => {
                Poll::Ready(Err(value))
            }
            _ => { Poll::Pending }
        }
    }
}

impl QPIncoming {
    pub async fn next(&mut self) -> Option<QPConnecting> {
        let result = self.0.next().await;

        if let Some(result) = result {
            return Some(QPConnecting(result));
        }

        None
    }
}

pub struct QPEndpoint(quinn::Endpoint);

impl Deref for QPEndpoint {
    type Target = quinn::Endpoint;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for QPEndpoint {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl QPEndpoint {
    pub fn server(
        config: ServerConfig,
        addr: SocketAddr
    ) -> io::Result<(Self, QPIncoming)> {
        let (endpoint, incoming) = quinn::Endpoint::server(config, addr)?;
        Ok((QPEndpoint(endpoint), QPIncoming(incoming)))
    }

    pub fn client(
        addr: SocketAddr
    ) -> io::Result<Self> {
        let endpoint = quinn::Endpoint::client(addr)?;
        Ok(QPEndpoint(endpoint))
    }

    pub fn connect(
        &self,
        addr: SocketAddr,
        server_name: &str
    ) -> Result<QPConnecting, quinn::ConnectError> {
        let connecting = self.0.connect(addr, server_name)?;
        Ok(QPConnecting(connecting))
    }

}

