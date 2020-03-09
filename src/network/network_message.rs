#[derive(Debug, Clone, Default, PartialEq)]
pub struct NetworkMessage {
    raw_msg: Vec<u8>,
    incoming: bool,
}

impl NetworkMessage {
    /// Create new incoming message
    pub fn incoming<T: AsRef<[u8]>>(msg: T) -> Self {
        Self::new(msg.as_ref(), true)
    }

    /// Create new outgoing message
    pub fn outgoing<T: AsRef<[u8]>>(msg: T) -> Self {
        Self::new(msg.as_ref(), false)
    }

    /// Check if this message was initialized.
    pub fn is_empty(&self) -> bool {
        self.raw_msg.is_empty()
    }

    /// Get raw message context
    pub fn raw_msg(&self) -> &[u8] {
        &self.raw_msg
    }

    /// Get mutable message context
    pub fn raw_msg_mut(&mut self) -> &mut [u8] {
        &mut self.raw_msg
    }

    /// Check if this message is incoming
    pub fn is_incoming(&self) -> bool {
        !self.is_empty() && self.incoming
    }

    /// Check if this message is outgoing
    pub fn is_outgoing(&self) -> bool {
        !self.is_empty() && !self.incoming
    }

    #[inline]
    /// Try to *guess*, if received message is a nonce exchange message. By current model, it is not
    /// easily possible to definitely decide, if message is part of the exchange or not.
    /// *Expect high false positive/negative rates on this method*
    pub fn is_nonce_message(&self) -> bool {
        let msg = &self.raw_msg;
        !self.is_empty() && msg.len() == 32 && msg[msg.len() - 4..msg.len()] == [1, 3, 3, 7]
    }

    fn new(msg: &[u8], incoming: bool) -> Self {
        Self {
            raw_msg: Vec::from(msg),
            incoming,
        }
    }
}