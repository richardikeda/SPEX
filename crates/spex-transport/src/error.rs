use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("libp2p error: {0}")]
    Libp2p(String),
    #[error("gossipsub publish error: {0}")]
    GossipPublish(String),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("inbox bridge returned invalid payload")]
    BridgePayload,
}

impl From<libp2p::gossipsub::PublishError> for TransportError {
    fn from(value: libp2p::gossipsub::PublishError) -> Self {
        Self::GossipPublish(value.to_string())
    }
}
