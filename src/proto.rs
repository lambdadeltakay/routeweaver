use crate::{error::RouteWeaverError, limited::LimitedVec};
use arrayvec::ArrayString;

use bincode::config::{BigEndian, Configuration, Limit, Varint};
use byte_unit::Unit;
use data_encoding::HEXLOWER_PERMISSIVE;
use itertools::Itertools;

use serde::{Deserialize, Serialize};

use std::{
    collections::HashSet, fmt::Display, mem::size_of, net::IpAddr, num::NonZeroU8, str::FromStr,
    time::Duration,
};
use zeroize::{Zeroize, ZeroizeOnDrop};

pub const MAX_MESSAGE_SEGMENT_SIZE: usize = 63 * Unit::KiB.as_bits_u128() as usize;
// Estimated size of a serialized packet
pub const MAX_SERIALIZED_PACKET_SIZE: usize =
    (size_of::<PublicKey>() * 2) + 64 * Unit::KiB.as_bits_u128() as usize + 100;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Address {
    Ip(IpAddr),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Zeroize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Tcp,
    Unix,
    Http,
    Bluetooth,
}

impl Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Protocol::Tcp => "tcp",
            Protocol::Unix => "unix",
            Protocol::Http => "http",
            Protocol::Bluetooth => "bluetooth",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Peer {
    pub protocol: Protocol,
    pub address: Address,
}

impl Display for Peer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}@{}",
            self.protocol,
            match &self.address {
                Address::Ip(ip) => ip.to_string(),
            }
        )?;

        Ok(())
    }
}

impl FromStr for Peer {
    type Err = RouteWeaverError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some((protocol, address)) = s.splitn(3, '@').collect_tuple() else {
            return Err(RouteWeaverError::PeerAddress);
        };

        match protocol.to_lowercase().as_str() {
            "tcp" => Ok(Peer {
                protocol: Protocol::Tcp,
                address: Address::Ip(address.parse().map_err(|_| RouteWeaverError::PeerAddress)?),
            }),
            _ => Err(RouteWeaverError::PeerAddress),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, ZeroizeOnDrop)]
pub struct PrivateKey(pub [u8; 32]);

impl Display for PrivateKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&HEXLOWER_PERMISSIVE.encode(&self.0))
    }
}

impl FromStr for PrivateKey {
    type Err = RouteWeaverError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(PrivateKey(
            HEXLOWER_PERMISSIVE
                .decode(s.as_bytes())
                .map_err(|_| RouteWeaverError::KeyParsingError)?
                .try_into()
                .map_err(|_| RouteWeaverError::KeyParsingError)?,
        ))
    }
}

#[derive(
    Serialize, Deserialize, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Zeroize,
)]
pub struct PublicKey(pub [u8; 32]);

impl Display for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&HEXLOWER_PERMISSIVE.encode(&self.0))
    }
}

impl FromStr for PublicKey {
    type Err = RouteWeaverError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(PublicKey(
            HEXLOWER_PERMISSIVE
                .decode(s.as_bytes())
                .map_err(|_| RouteWeaverError::KeyParsingError)?
                .try_into()
                .map_err(|_| RouteWeaverError::KeyParsingError)?,
        ))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Packet {
    pub source: PublicKey,
    pub destination: PublicKey,
    pub message: MessageSegment,
}

#[derive(Serialize, Deserialize, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Zeroize)]
pub struct ApplicationId(pub ArrayString<8>);

#[derive(Serialize, Deserialize, Debug)]
pub enum MessageSegment {
    Message {
        index: u8,
        data: LimitedVec<u8, MAX_MESSAGE_SEGMENT_SIZE>,
    },
    EndMessage {
        compression_mode: Option<MessageCompressionMode>,
        total_indexes: NonZeroU8,
        hash: [u8; 32],
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum MessageCompressionMode {
    Lz4,
    Zlib
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Message {
    Denied,
    Handshake,
    RequestPeersList,
    PeersList {
        peers: HashSet<Peer>,
    },
    RequestSystemInformation,
    SystemInformation {
        compute_max_time: Option<Duration>,
    },
    RequestApplicationAdvertisement,
    ApplicationAdvertisement {
        applications: HashSet<ApplicationId>,
    },
}

pub const BINCODE_PACKET_CONFIG: Configuration<
    BigEndian,
    Varint,
    Limit<MAX_SERIALIZED_PACKET_SIZE>,
> = bincode::config::standard()
    .with_big_endian()
    .with_variable_int_encoding()
    .with_limit::<MAX_SERIALIZED_PACKET_SIZE>();

pub const BINCODE_MESSAGE_CONFIG: Configuration<
    BigEndian,
    Varint,
    Limit<MAX_MESSAGE_SEGMENT_SIZE>,
> = bincode::config::standard()
    .with_big_endian()
    .with_variable_int_encoding()
    .with_limit::<MAX_MESSAGE_SEGMENT_SIZE>();
