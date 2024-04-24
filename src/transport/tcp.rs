use super::{PlainBincodePacketReader, PlainBincodePacketWriter, Transport};
use crate::{
    config::TransportConfig,
    error::RouteWeaverError,
    proto::{Address, Protocol},
};
use socket2::Socket;
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};
use tokio::net::{
    tcp::{OwnedReadHalf, OwnedWriteHalf},
    TcpListener, TcpStream,
};

pub struct TcpTransport {
    socket: TcpListener,
}

impl Transport for TcpTransport {
    const PROTOCOL: Protocol = Protocol::Tcp;

    type Reader = PlainBincodePacketReader<OwnedReadHalf>;
    type Writer = PlainBincodePacketWriter<OwnedWriteHalf>;

    async fn new(_config: Option<&TransportConfig>) -> Result<Self, RouteWeaverError>
    where
        Self: Sized,
    {
        let socket = Socket::new(
            socket2::Domain::IPV6,
            socket2::Type::STREAM,
            Some(socket2::Protocol::TCP),
        )?;

        socket.set_only_v6(false).unwrap();
        socket.set_nonblocking(true).unwrap();
        socket.set_reuse_address(true).unwrap();

        socket
            .bind(&SocketAddr::new("::".parse().unwrap(), 3434).into())
            .unwrap();

        socket.listen(4).unwrap();

        Ok(Self {
            socket: TcpListener::from_std(std::net::TcpListener::from(socket)).unwrap(),
        })
    }

    async fn connect(
        self: Arc<Self>,
        address: Option<&Address>,
    ) -> Result<(Self::Reader, Self::Writer), RouteWeaverError> {
        let ip_addr = address
            .and_then(|addr| {
                if let Address::Ip(ip) = addr {
                    return Some(*ip);
                }

                None
            })
            .expect("Need IP");

        let addr = SocketAddr::new(ip_addr, 3434);

        Ok(TcpStream::connect(addr).await.map(|stream| {
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
        Ok(self.socket.accept().await.map(|(stream, address)| {
            let (read, write) = stream.into_split();
            (
                (
                    PlainBincodePacketReader::new(read),
                    PlainBincodePacketWriter::new(write),
                ),
                Some(Address::Ip({
                    match address.ip() {
                        IpAddr::V4(ip) => IpAddr::V4(ip),
                        IpAddr::V6(ip) => ip
                            .to_ipv4_mapped()
                            .map_or_else(|| IpAddr::V6(ip), IpAddr::V4),
                    }
                })),
            )
        })?)
    }
}
