use crate::storage::StoreMessage;
use tezos_messages::p2p::encoding::peer::PeerMessage;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Default)]
/// Track requests for last given
pub struct RequestTracker {
    swap_request: Option<u64>,
    current_branch_request: Option<u64>,
    current_head: Option<u64>,
    block_header_request: Option<u64>,
    operations_request: Option<u64>,
    protocols_request: Option<u64>,
    operation_hashes_for_blocks: Option<u64>,
    operations_for_blocks: Option<u64>,
}

impl RequestTracker {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn track_request(&mut self, msg: &mut StoreMessage, msg_id: u64) {
        match msg {
            StoreMessage::P2PMessage { request_id, payload, .. } => {
                let msg = payload.first();
                let id = Some(msg_id);
                if let Some(msg) = msg {
                    match msg {
                        PeerMessage::SwapRequest(_) => {
                            self.swap_request = id;
                            *request_id = id;
                        }
                        PeerMessage::SwapAck(_) => {
                            *request_id = self.swap_request;
                        }
                        PeerMessage::GetCurrentBranch(_) => {
                            self.current_branch_request = id;
                            *request_id = id;
                        }
                        PeerMessage::CurrentBranch(_) => {
                            *request_id = self.current_branch_request;
                        }
                        PeerMessage::GetCurrentHead(_) => {
                            self.current_head = id;
                            *request_id = id;
                        }
                        PeerMessage::CurrentHead(_) => {
                            *request_id = self.current_head;
                        }
                        PeerMessage::GetBlockHeaders(_) => {
                            self.block_header_request = id;
                            *request_id = id;
                        }
                        PeerMessage::BlockHeader(_) => {
                            *request_id = self.block_header_request;
                        }
                        PeerMessage::GetOperations(_) => {
                            self.operations_request = id;
                            *request_id = id;
                        }
                        PeerMessage::Operation(_) => {
                            *request_id = self.operations_request;
                        }
                        PeerMessage::GetProtocols(_) => {
                            self.protocols_request = id;
                            *request_id = id;
                        }
                        PeerMessage::Protocol(_) => {
                            *request_id = self.protocols_request;
                        }
                        PeerMessage::GetOperationHashesForBlocks(_) => {
                            self.operation_hashes_for_blocks = id;
                            *request_id = id;
                        }
                        PeerMessage::OperationHashesForBlock(_) => {
                            *request_id = self.operation_hashes_for_blocks;
                        }
                        PeerMessage::GetOperationsForBlocks(_) => {
                            self.operations_for_blocks = id;
                            *request_id = id;
                        }
                        PeerMessage::OperationsForBlocks(_) => {
                            *request_id = self.operations_for_blocks;
                        }
                        _ => return,
                    }
                }
            }
            _ => return,
        }
    }
}