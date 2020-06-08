use crate::storage::StoreMessage;
use tezos_messages::p2p::encoding::peer::PeerMessage;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Default)]
pub struct RequestTrack {
    pub request_id: u64,
    pub incoming: bool,
}

impl RequestTrack {
    pub fn new(request_id: u64, incoming: bool) -> Self {
        Self { request_id, incoming }
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Default)]
/// Track requests for last given
pub struct RequestTracker {
    swap_request: Option<RequestTrack>,
    current_branch_request: Option<RequestTrack>,
    current_head: Option<RequestTrack>,
    block_header_request: Option<RequestTrack>,
    operations_request: Option<RequestTrack>,
    protocols_request: Option<RequestTrack>,
    operation_hashes_for_blocks: Option<RequestTrack>,
    operations_for_blocks: Option<RequestTrack>,
}

impl RequestTracker {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn track_request(&mut self, msg: &mut StoreMessage, msg_id: u64) {
        match msg {
            StoreMessage::P2PMessage { incoming, request_id, remote_requested, payload, .. } => {
                let msg = payload.first();
                let id = Some(RequestTrack::new(msg_id, *incoming));
                if let Some(msg) = msg {
                    match msg {
                        PeerMessage::SwapRequest(_) => {
                            self.swap_request = id;
                            *request_id = Some(msg_id);
                            *remote_requested = Some(*incoming)
                        }
                        PeerMessage::SwapAck(_) => {
                            if let Some(rt) = self.swap_request {
                                *request_id = Some(rt.request_id);
                                *remote_requested = Some(rt.incoming);
                            } else {
                                *remote_requested = Some(!*incoming);
                            }
                        }
                        PeerMessage::GetCurrentBranch(_) => {
                            self.current_branch_request = id;
                            *request_id = Some(msg_id);
                            *remote_requested = Some(*incoming)
                        }
                        PeerMessage::CurrentBranch(_) => {
                            if let Some(rt) = self.current_branch_request {
                                *request_id = Some(rt.request_id);
                                *remote_requested = Some(rt.incoming);
                            } else {
                                *remote_requested = Some(!*incoming);
                            }
                        }
                        PeerMessage::GetCurrentHead(_) => {
                            self.current_head = id;
                            *request_id = Some(msg_id);
                            *remote_requested = Some(*incoming)
                        }
                        PeerMessage::CurrentHead(_) => {
                            if let Some(rt) = self.current_head {
                                *request_id = Some(rt.request_id);
                                *remote_requested = Some(rt.incoming);
                            } else {
                                *remote_requested = Some(!*incoming);
                            }
                        }
                        PeerMessage::GetBlockHeaders(_) => {
                            self.block_header_request = id;
                            *request_id = Some(msg_id);
                            *remote_requested = Some(*incoming)
                        }
                        PeerMessage::BlockHeader(_) => {
                            if let Some(rt) = self.block_header_request {
                                *request_id = Some(rt.request_id);
                                *remote_requested = Some(rt.incoming);
                            } else {
                                *remote_requested = Some(!*incoming);
                            }
                        }
                        PeerMessage::GetOperations(_) => {
                            self.operations_request = id;
                            *request_id = Some(msg_id);
                            *remote_requested = Some(*incoming)
                        }
                        PeerMessage::Operation(_) => {
                            if let Some(rt) = self.operations_request {
                                *request_id = Some(rt.request_id);
                                *remote_requested = Some(rt.incoming);
                            } else {
                                *remote_requested = Some(!*incoming);
                            }
                        }
                        PeerMessage::GetProtocols(_) => {
                            self.protocols_request = id;
                            *request_id = Some(msg_id);
                            *remote_requested = Some(*incoming)
                        }
                        PeerMessage::Protocol(_) => {
                            if let Some(rt) = self.protocols_request {
                                *request_id = Some(rt.request_id);
                                *remote_requested = Some(rt.incoming);
                            } else {
                                *remote_requested = Some(!*incoming);
                            }
                        }
                        PeerMessage::GetOperationHashesForBlocks(_) => {
                            self.operation_hashes_for_blocks = id;
                            *request_id = Some(msg_id);
                            *remote_requested = Some(*incoming)
                        }
                        PeerMessage::OperationHashesForBlock(_) => {
                            if let Some(rt) = self.operation_hashes_for_blocks {
                                *request_id = Some(rt.request_id);
                                *remote_requested = Some(rt.incoming);
                            } else {
                                *remote_requested = Some(!*incoming);
                            }
                        }
                        PeerMessage::GetOperationsForBlocks(_) => {
                            self.operations_for_blocks = id;
                            *request_id = Some(msg_id);
                            *remote_requested = Some(*incoming)
                        }
                        PeerMessage::OperationsForBlocks(_) => {
                            if let Some(rt) = self.operations_for_blocks {
                                *request_id = Some(rt.request_id);
                                *remote_requested = Some(rt.incoming);
                            } else {
                                *remote_requested = Some(!*incoming);
                            }
                        }
                        _ => return,
                    }
                }
            }
            _ => return,
        }
    }
}