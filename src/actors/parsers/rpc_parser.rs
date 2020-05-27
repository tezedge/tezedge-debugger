use failure::Error;
use riker::actors::*;
use std::collections::HashMap;
use crate::utility::tcp_packet::{Packet, IdAddrs};
use crate::utility::http_message::{RequestParser, ResponseParser, HttpMessage, RPCMessage};
use crate::utility::http_message::http::is_request;

#[derive(Debug, Default, Clone)]
pub struct RPCParser {
    port: u16,
    requests: HashMap<IdAddrs, RequestParser>,
    responses: HashMap<IdAddrs, ResponseParser>,
}

impl RPCParser {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            ..Default::default()
        }
    }

    fn get_request_parser(&mut self, addr: IdAddrs) -> &mut RequestParser {
        let parser = self.requests.entry(addr);
        parser.or_insert(RequestParser::new())
    }

    fn get_response_parser(&mut self, addr: IdAddrs) -> &mut ResponseParser {
        let parser = self.responses.entry(addr);
        parser.or_insert(ResponseParser::new())
    }

    fn process_message(&mut self, msg: &Packet) -> Result<Option<HttpMessage>, Error> {
        let is_request = is_request(msg.payload());
        Ok(if let Some(is_request) = is_request {
            if is_request {
                let parser = self.get_request_parser(msg.identification_pair());
                let res = parser.process_message(msg.payload());
                if res.is_some() {
                    self.requests.remove(&msg.identification_pair());
                }
                res
            } else {
                let parser = self.get_response_parser(msg.identification_pair());
                let res = parser.process_message(msg.payload());
                if res.is_some() {
                    self.responses.remove(&msg.identification_pair());
                }
                res
            }
        } else {
            log::warn!("Received invalid HTTP message: {:?}", msg.payload());
            None
        })
    }
}

impl ActorFactoryArgs<u16> for RPCParser {
    fn create_args(args: u16) -> Self {
        Self::new(args)
    }
}

impl Actor for RPCParser {
    type Msg = Packet;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, _: Sender) {
        match self.process_message(&msg) {
            Ok(Some(parsed)) => {
                match ctx.select("/user/processors/*") {
                    Ok(actor_ref) => {
                        let msg = if parsed.is_response() {
                            RPCMessage::new(parsed, msg.source_addr())
                        } else {
                            RPCMessage::new(parsed, msg.destination_address())
                        };
                        actor_ref.try_tell(msg, ctx.myself());
                    }
                    Err(err) => {
                        log::error!("Failed to propagate parsed HTTP message: {}", err);
                    }
                }
            }
            Err(err) => {
                log::error!("Failed at processing TCP packet as HTTP message: {}", err);
            }
            Ok(None) => {
                // It successfully processed the message, but it is not complete yet. Do nothing.
            }
        }
    }
}

