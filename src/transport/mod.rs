#[cfg(tcp_transport)]
pub mod tcp;
#[cfg(unix_transport)]
pub mod unix;

use crate::{
    config::TransportConfig,
    error::RouteWeaverError,
    proto::{Address, Packet, Protocol, BINCODE_PACKET_CONFIG},
};
use bincode::{
    error::DecodeError,
    serde::{decode_from_std_read, encode_into_std_write},
};
use bytes::{buf::Writer, BytesMut};
use data_encoding::BASE64_NOPAD;
use futures_util::{Sink, SinkExt, Stream};
use std::pin::{pin, Pin};
use std::{fmt::Debug, sync::Arc};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::{
    bytes::{Buf, BufMut},
    codec::{Decoder, Encoder, FramedRead, FramedWrite},
};

pub trait TransportWriter: Send + Sync + Unpin + Sink<Packet, Error = RouteWeaverError> {}

pub trait TransportReader:
    Send + Sync + Unpin + Stream<Item = Result<Packet, RouteWeaverError>>
{
}

pub trait Transport: Send + Sync + 'static {
    const PROTOCOL: Protocol;

    type Reader: TransportReader;
    type Writer: TransportWriter;

    async fn new(config: Option<&TransportConfig>) -> Result<Self, RouteWeaverError>
    where
        Self: Sized;

    async fn connect(
        self: Arc<Self>,
        address: Option<&Address>,
    ) -> Result<(Self::Reader, Self::Writer), RouteWeaverError>;

    async fn accept(
        self: Arc<Self>,
    ) -> Result<((Self::Reader, Self::Writer), Option<Address>), RouteWeaverError>;

    fn recommended_message_segment_size(&self) -> Option<usize> {
        None
    }
}

#[derive(Debug)]
pub struct PlainBincodePacketWriter<T: AsyncWrite + Debug + Send + Sync + Unpin> {
    writer: FramedWrite<T, PacketEncoderDecoder>,
}

impl<T: AsyncWrite + Debug + Send + Sync + Unpin> PlainBincodePacketWriter<T> {
    pub fn new(writer: T) -> Self {
        Self {
            writer: FramedWrite::new(writer, PacketEncoderDecoder),
        }
    }
}

impl<T: AsyncWrite + Debug + Send + Sync + Unpin> TransportWriter for PlainBincodePacketWriter<T> {}

impl<T: AsyncWrite + Debug + Send + Sync + Unpin> Sink<Packet> for PlainBincodePacketWriter<T> {
    type Error = RouteWeaverError;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        pin!(&mut self.writer).poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Packet) -> Result<(), Self::Error> {
        pin!(&mut self.writer).start_send(item)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        pin!(&mut self.writer).poll_flush(cx)
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        pin!(&mut self.writer).poll_close(cx)
    }
}

#[derive(Debug)]
pub struct PlainBincodePacketReader<T: AsyncRead + Debug + Send + Sync + Unpin> {
    reader: FramedRead<T, PacketEncoderDecoder>,
}

impl<T: AsyncRead + Debug + Send + Sync + Unpin> PlainBincodePacketReader<T> {
    pub fn new(reader: T) -> Self {
        Self {
            reader: FramedRead::new(reader, PacketEncoderDecoder),
        }
    }
}

impl<T: AsyncRead + Debug + Send + Sync + Unpin> TransportReader for PlainBincodePacketReader<T> {}

impl<T: AsyncRead + Debug + Send + Sync + Unpin> Stream for PlainBincodePacketReader<T> {
    type Item = Result<Packet, RouteWeaverError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        pin!(&mut self.reader).poll_next(cx)
    }
}

#[derive(Default, Debug)]
pub struct PacketEncoderDecoder;

impl Decoder for PacketEncoderDecoder {
    type Item = Packet;
    type Error = RouteWeaverError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.is_empty() {
            return Ok(None);
        }

        // On purpose our packets don't have any magic bytes

        match decode_from_std_read(&mut src.reader(), BINCODE_PACKET_CONFIG) {
            Ok(packet) => Ok(Some(packet)),
            Err(e) => match e {
                DecodeError::UnexpectedEnd { additional } => {
                    log::trace!("Not enough bytes to decode packet: {}", additional);
                    // We haven't gotten enough bytes to deserialize
                    Ok(None)
                }
                _ => Err(RouteWeaverError::PacketDecoding(e)),
            },
        }
    }
}

impl Encoder<Packet> for PacketEncoderDecoder {
    type Error = RouteWeaverError;

    fn encode(&mut self, item: Packet, dst: &mut BytesMut) -> Result<(), Self::Error> {
        if encode_into_std_write(item, &mut dst.writer(), BINCODE_PACKET_CONFIG).is_err() {
            return Err(RouteWeaverError::PacketEncoding);
        }

        Ok(())
    }
}
