use bincode::serde::encode_to_vec;
use blake2::{Blake2s256, Digest};
use dashmap::DashMap;
use deadqueue::unlimited::Queue;
use entropy::shannon_entropy;
use futures_util::StreamExt;
use itertools::Itertools;
use scc::HashCache;
use std::{
    io::Cursor,
    pin::{pin, Pin},
    sync::Arc,
};
use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::{
    config::Config,
    limited::LimitedVec,
    proto::{
        Message, MessageCompressionMode, MessageSegment, PublicKey, BINCODE_MESSAGE_CONFIG,
        MAX_MESSAGE_SEGMENT_SIZE,
    },
    transport::{Transport, TransportReader},
};

pub fn determine_compression_for_data(data: &[u8]) -> Option<MessageCompressionMode> {
    if shannon_entropy(data) > 0.5 {
        None
    } else {
        Some(MessageCompressionMode::Lz4)
    }
}

#[derive(Debug)]
pub struct ClearTextMessage {
    pub destination: PublicKey,
    pub message: Message,
}

pub async fn encode_clear_text_message(
    my_public_key: PublicKey,
    message_queue: Arc<Queue<ClearTextMessage>>,
    message_sender: Sender<EncodedMessage>,
) {
    loop {
        let message = message_queue.pop().await;
        let message = encode_to_vec(&message.message, BINCODE_MESSAGE_CONFIG).unwrap();

        let tracked_compression_mode = determine_compression_for_data(&message);
        let mut tracked_index = 0;

        let message = Cursor::new(match tracked_compression_mode {
            Some(MessageCompressionMode::Lz4) => lz4_flex::compress_prepend_size(&message),
            Some(MessageCompressionMode::Zlib) => {
                miniz_oxide::deflate::compress_to_vec_zlib(&message, 10)
            }
            None => message,
        });
    }
}

pub async fn accept_connections_from_peers<T: Transport>(
    message_queue: Arc<Queue<ClearTextMessage>>,
    message_tracker: PreAssembledMessageTracker,
    config: Arc<Config>,
) {
    let transport_config = config.transport_configs.get(&T::PROTOCOL);
    let transport = Arc::new(T::new(transport_config).await.unwrap());

    while let Ok(((reader, _writer), address)) = transport.clone().accept().await {
        if let Some(address) = address {
            log::info!("Received connection from: {:?} on {}", address, T::PROTOCOL);
        } else {
            log::info!("Received connection on {}", T::PROTOCOL);
        }

        let (encoded_message_sender, encoded_message_receiver) = channel(1024);

        tokio::spawn(encode_clear_text_message(
            config.public_key,
            message_queue.clone(),
            encoded_message_sender.clone(),
        ));

        tokio::spawn(route_encoded_message(
            transport.clone(),
            encoded_message_receiver,
        ));

        tokio::spawn(packet_listener::<T>(
            reader,
            encoded_message_sender.clone(),
            message_tracker.clone(),
        ));
    }
}

pub type PreAssembledMessageTracker =
    Arc<HashCache<(PublicKey, PublicKey), DashMap<u8, LimitedVec<u8, MAX_MESSAGE_SEGMENT_SIZE>>>>;

#[derive(Debug)]
pub struct EncodedMessage {
    pub claimed_source: PublicKey,
    pub claimed_destination: PublicKey,
    pub compression_mode: Option<MessageCompressionMode>,
    pub message: Vec<u8>,
}

pub async fn route_encoded_message<T: Transport>(
    transport: Arc<T>,
    mut encoded_message_receiver: Receiver<EncodedMessage>,
) {
    while let Some(complete_message) = encoded_message_receiver.recv().await {
        log::info!(
            "received complete message from {:?} to {:?}",
            complete_message.claimed_source,
            complete_message.claimed_destination
        );
    }
}

pub async fn packet_listener<T: Transport>(
    reader: T::Reader,
    complete_message_sender: Sender<EncodedMessage>,
    pre_assembled_message_tracker: PreAssembledMessageTracker,
) {
    let mut reader = Box::pin(reader);

    while let Some(packet) = reader.next().await {
        match packet {
            Ok(packet) => {
                // Match the message segment type
                match packet.message {
                    // It's the actual data for the message
                    MessageSegment::Message { index, data } => {
                        let (_, message) = pre_assembled_message_tracker
                            .entry((packet.source, packet.destination))
                            .or_default();
                        let message = message.get();

                        if message.contains_key(&index) {
                            log::warn!(
                                "Received duplicate message segment from: {}",
                                packet.source
                            );
                        }

                        message.insert(index, data);
                    }
                    MessageSegment::EndMessage {
                        total_indexes,
                        hash,
                        compression_mode,
                    } => {
                        if let Some((_, message)) = pre_assembled_message_tracker
                            .remove(&(packet.source, packet.destination))
                        {
                            let stored_length = message.len();
                            if stored_length != total_indexes.get() as usize {
                                log::error!(
                                    "Mismatch in message segment count, expected: {}, actual: {}",
                                    total_indexes,
                                    message.len()
                                );
                                continue;
                            }

                            let sorted_message =
                                message.into_iter().sorted_by_key(|(x, _)| *x).collect_vec();

                            let mut hasher = Blake2s256::default();
                            for (_, segment) in &sorted_message {
                                hasher.update(&segment.0);
                            }
                            if hasher.finalize().as_slice() != hash {
                                log::error!("Message hash does not match");
                                continue;
                            }

                            let mut final_buffer = Vec::new();
                            for (_, segment) in sorted_message {
                                final_buffer.extend_from_slice(&segment.0);
                            }

                            complete_message_sender
                                .send(EncodedMessage {
                                    claimed_source: packet.source,
                                    claimed_destination: packet.destination,
                                    compression_mode,
                                    message: final_buffer,
                                })
                                .await
                                .unwrap();

                            log::info!(
                                "Complete message sent successfully from {} to {}",
                                packet.source,
                                packet.destination
                            );
                        } else {
                            log::error!(
                                "Received end message without start message from: {}",
                                packet.source
                            );
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Error reading packet: {}", e);
            }
        }
    }
}
