use crate::storage::StoreMessage;
use tezos_messages::p2p::encoding::peer::PeerMessage;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Default)]
/// Track requests for last given
pub struct RequestTracker {}

impl RequestTracker {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn track_request(&mut self, msg: &mut StoreMessage, _msg_id: u64) {
        match msg {
            StoreMessage::Metadata { incoming, source_type, .. } | StoreMessage::ConnectionMessage { incoming, source_type, .. } => *source_type = Some(*incoming),
            StoreMessage::P2PMessage { incoming, source_type, payload, .. } => {
                if let Some(payload) = payload.first() {
                    match payload {
                        PeerMessage::Disconnect | PeerMessage::Advertise(_) | PeerMessage::Bootstrap | PeerMessage::SwapRequest(_)
                        | PeerMessage::GetCurrentBranch(_) | PeerMessage::Deactivate(_) | PeerMessage::GetCurrentHead(_)
                        | PeerMessage::GetBlockHeaders(_) | PeerMessage::GetOperations(_) | PeerMessage::GetProtocols(_)
                        | PeerMessage::GetOperationHashesForBlocks(_) | PeerMessage::GetOperationsForBlocks(_) => *source_type = Some(*incoming),
                        PeerMessage::SwapAck(_) | PeerMessage::CurrentBranch(_) | PeerMessage::CurrentHead(_)
                        | PeerMessage::BlockHeader(_) | PeerMessage::Operation(_) | PeerMessage::Protocol(_)
                        | PeerMessage::OperationHashesForBlock(_) | PeerMessage::OperationsForBlocks(_) => *source_type = Some(!*incoming),
                    }
                }
            }
            _ => return,
        }
    }
}