use thiserror::Error;

#[derive(Error, Debug)]
pub enum RouteWeaverError {
    #[error("io error: {0}")]
    Standard(#[from] std::io::Error),
    #[error("compute module error")]
    IncorrectComputeModuleBehavior,
    #[error("packet encoding error")]
    PacketEncoding,
    #[error("packet decoding error {0}")]
    PacketDecoding(#[from] bincode::error::DecodeError),
    #[error("transport connection error")]
    TransportConnection,
    #[error("peer address error")]
    PeerAddress,
    #[error("key parsing error")]
    KeyParsingError,
}
