use super::{PlainBincodePacketReader, PlainBincodePacketWriter, Transport};
use crate::{
    config::TransportConfig,
    error::RouteWeaverError,
    proto::{Address, Protocol},
};
use std::{env::temp_dir, fs::remove_file, path::PathBuf, sync::Arc};
use tokio::net::{
    unix::{OwnedReadHalf, OwnedWriteHalf},
    UnixListener, UnixStream,
};

pub struct UnixTransport {
    path: PathBuf,
    socket: UnixListener,
}

impl Transport for UnixTransport {
    const PROTOCOL: Protocol = Protocol::Unix;

    type Reader = PlainBincodePacketReader<OwnedReadHalf>;
    type Writer = PlainBincodePacketWriter<OwnedWriteHalf>;

    async fn new(config: Option<&TransportConfig>) -> Result<Self, RouteWeaverError>
    where
        Self: Sized,
    {
        let path = config
            .and_then(|config| {
                config
                    .get("socket_path")
                    .and_then(|value| value.as_str().map(PathBuf::from)?.canonicalize().ok())
            })
            .unwrap_or_else(|| temp_dir().join("routeweaver-unix-transport"));

        // If it fails I don't really care
        let _ = remove_file(&path);

        Ok(Self {
            socket: UnixListener::bind(&path)?,
            path,
        })
    }

    async fn connect(
        self: Arc<Self>,
        _address: Option<&Address>,
    ) -> Result<(Self::Reader, Self::Writer), RouteWeaverError> {
        Ok(UnixStream::connect(&self.path).await.map(|stream| {
            let (read, write) = stream.into_split();

            (
                PlainBincodePacketReader::new(read),
                PlainBincodePacketWriter::new(write),
            )
        })?)
    }

    async fn accept(
        self: Arc<Self>,
    ) -> Result<((Self::Reader, Self::Writer), Option<Address>), RouteWeaverError> {
        Ok(self.socket.accept().await.map(|(stream, _)| {
            let (read, write) = stream.into_split();
            (
                (
                    PlainBincodePacketReader::new(read),
                    PlainBincodePacketWriter::new(write),
                ),
                None,
            )
        })?)
    }
}
