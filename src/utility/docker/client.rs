// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tokio::{net::TcpStream, io, stream::StreamExt};
use serde::de::DeserializeOwned;
use std::net::SocketAddr;

use super::{container::Container, stat::Stat, top::Top};

pub trait Captures<'a> {}

impl<'a, T: ?Sized> Captures<'a> for T {}

/// Far from complete docker client
/// https://docs.docker.com/engine/api/
pub struct DockerClient {
    inner: TcpStream,
}

impl DockerClient {
    const API_VERSION: &'static str = "v1.40";

    pub async fn connect(addr: SocketAddr) -> Result<Self, io::Error> {
        Ok(DockerClient {
            inner: TcpStream::connect(addr).await?,
        })
    }

    async fn get<T>(&mut self, req: String) -> Result<T, io::Error>
    where
        T: DeserializeOwned,
    {
        self.stream(req)
            .await
            .next()
            .await
            .ok_or(io::Error::new(
                io::ErrorKind::InvalidData,
                "Failed to parse JSON",
            ))
            .and_then(|x| x)
    }

    async fn stream<'a, T>(
        &'a mut self,
        req: String,
    ) -> impl Captures<'a> + StreamExt<Item = Result<T, io::Error>>
    where
        T: DeserializeOwned,
    {
        use tokio::io::AsyncWriteExt;
        use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};

        self.inner
            .write(
                format!(
                    "GET {} HTTP/1.1\r\nHost: {}\r\n\r\n",
                    req,
                    Self::API_VERSION
                )
                .as_bytes(),
            )
            .await
            .unwrap();

        Framed::new(&mut self.inner, LinesCodec::new()).filter_map(|s| match s {
            Ok(s) => {
                // TODO: fix it, need to ignore HTTP response header and fetch only json
                // println!("{}", s);
                if s.starts_with("{") || s.starts_with('[') {
                    let t = serde_json::from_str(&s)
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e));
                    Some(t)
                } else {
                    let _ = s;

                    // nothing interesting here,
                    // just empty strings, lengths of the content (in hex, e.g. a48)
                    // and strings like:

                    // HTTP/1.1 200 OK
                    // Api-Version: 1.40
                    // Docker-Experimental: false
                    // Ostype: linux
                    // Server: Docker/19.03.12-ce (linux)
                    // Date: Tue, 04 Aug 2020 17:38:39 GMT
                    // Transfer-Encoding: chunked

                    None
                }
            },
            Err(LinesCodecError::MaxLineLengthExceeded) => Some(Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Max line length exceeded",
            ))),
            Err(LinesCodecError::Io(io)) => Some(Err(io)),
        })
    }

    pub async fn list_containers(&mut self) -> Result<Vec<Container>, io::Error> {
        self.get("/containers/json?size=true".to_owned()).await
    }

    pub async fn top(&mut self, container_id: &str, ps_args: &str) -> Result<Top, io::Error> {
        self.get(format!("/containers/{}/top?ps_args={}", container_id, ps_args)).await
    }

    pub async fn stats<'a>(
        &'a mut self,
        container_id: &str,
    ) -> impl Captures<'a> + StreamExt<Item = Result<Stat, io::Error>> {
        self.stream::<Stat>(format!("/containers/{}/stats", container_id))
            .await
    }

    pub async fn stats_single<'a>(
        &'a mut self,
        container_id: &str,
    ) -> Result<Stat, io::Error> {
        self.get::<Stat>(format!("/containers/{}/stats?stream=false", container_id))
            .await
    }
}