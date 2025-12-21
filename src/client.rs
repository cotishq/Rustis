use std::collections::HashSet;
use crate::resp::Value;

pub struct ClientState {
    pub subscribed_channels: HashSet<String>,
    pub in_transaction: bool,
    pub queued_commands: Vec<(String, Vec<Value>)>,
}

impl ClientState {
    pub fn new() -> Self {
        Self {
            subscribed_channels: HashSet::new(),
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
