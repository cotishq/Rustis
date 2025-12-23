use std::collections::HashSet;
use tokio::sync::mpsc;
use crate::resp::Value;
use crate::db::PubSubMessage;

pub struct ClientState {
    pub subscribed_channels: HashSet<String>,
    pub pubsub_tx: mpsc::Sender<PubSubMessage>,
    pub pubsub_rx: mpsc::Receiver<PubSubMessage>,
    pub in_transaction: bool,
    pub queued_commands: Vec<(String, Vec<Value>)>,
}

impl ClientState {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(100);
        Self {
            subscribed_channels: HashSet::new(),
            pubsub_tx: tx,
            pubsub_rx: rx,
            in_transaction: false,
            queued_commands: Vec::new(),
        }
    }

    pub fn is_subscribed(&self) -> bool {
        !self.subscribed_channels.is_empty()
    }
}

impl Default for ClientState {
    fn default() -> Self {
        Self::new()
    }
}
