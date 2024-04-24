use std::{collections::BTreeMap, convert::Infallible, pin::Pin, sync::Arc};

use crate::{
    config::Config,
    limited::LimitedVec,
    proto::{
        Message, MessageCompressionMode, MessageSegment, PrivateKey, PublicKey,
        BINCODE_MESSAGE_CONFIG, MAX_MESSAGE_SEGMENT_SIZE,
    },
    transport::{Transport, TransportReader},
};
use bincode::serde::encode_to_vec;
use blake2::{Blake2s256, Digest};
use dashmap::DashMap;
use deadqueue::limited::Queue;
use futures_util::StreamExt;
use itertools::Itertools;
use once_cell::sync::Lazy;
use scc::HashCache;
use snow::{params::NoiseParams, HandshakeState, TransportState};
use tokio::sync::mpsc::{channel, Receiver, Sender};

static NOISE_PROLOGUE: Lazy<String> =
    Lazy::new(|| format!("router-weaver edition {}", env!("CARGO_PKG_VERSION_MAJOR")));

static NOISE_PATTERN: Lazy<NoiseParams> =
    Lazy::new(|| "Noise_XX_25519_ChaChaPoly_BLAKE2s".parse().unwrap());

fn create_noise_builder<'a>() -> snow::Builder<'a> {
    snow::Builder::new(NOISE_PATTERN.clone()).prologue(NOISE_PROLOGUE.as_bytes())
}

pub fn create_keypair() -> (PublicKey, PrivateKey) {
    let keypair = create_noise_builder().generate_keypair().unwrap();

    (
        PublicKey(keypair.public.try_into().unwrap()),
        PrivateKey(keypair.private.try_into().unwrap()),
    )
}

fn create_responder(key: &PrivateKey) -> NoiseState {
    NoiseState::Handshake(Box::new(
        create_noise_builder()
            .local_private_key(&key.0)
            .build_responder()
            .unwrap(),
    ))
}

fn create_initiator(key: &PrivateKey) -> NoiseState {
    NoiseState::Handshake(Box::new(
        create_noise_builder()
            .local_private_key(&key.0)
            .build_initiator()
            .unwrap(),
    ))
}

pub enum NoiseState {
    Handshake(Box<HandshakeState>),
    Transport(Box<TransportState>),
}

