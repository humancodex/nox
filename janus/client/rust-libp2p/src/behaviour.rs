/*
 * Copyright 2019 Fluence Labs Limited
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use faas_api::{FunctionCall, ProtocolMessage};
use failure::_core::task::{Context, Poll};
use janus_libp2p::generate_swarm_event_type;
use libp2p::core::connection::ConnectionId;
use libp2p::core::either::EitherOutput;
use libp2p::ping::{Ping, PingConfig, PingResult};
use libp2p::swarm::{
    IntoProtocolsHandler, IntoProtocolsHandlerSelect, NetworkBehaviour, NetworkBehaviourAction,
    NotifyHandler, OneShotHandler, PollParameters,
};
use libp2p::PeerId;
use parity_multiaddr::Multiaddr;
use std::collections::VecDeque;

pub type SwarmEventType = generate_swarm_event_type!(ClientBehaviour);

pub struct ClientBehaviour {
    events: VecDeque<SwarmEventType>,
    ping: Ping,
}

impl Default for ClientBehaviour {
    fn default() -> Self {
        let ping = Ping::new(PingConfig::new().with_keep_alive(true));
        Self {
            events: VecDeque::default(),
            ping,
        }
    }
}

impl ClientBehaviour {
    pub fn call(&mut self, peer_id: PeerId, call: FunctionCall) {
        self.events
            .push_back(NetworkBehaviourAction::NotifyHandler {
                event: EitherOutput::First(ProtocolMessage::FunctionCall(call)),
                handler: NotifyHandler::Any,
                peer_id,
            })
    }
}

impl NetworkBehaviour for ClientBehaviour {
    type ProtocolsHandler = IntoProtocolsHandlerSelect<
        OneShotHandler<ProtocolMessage, ProtocolMessage, ProtocolMessage>,
        <Ping as NetworkBehaviour>::ProtocolsHandler,
    >;

    type OutEvent = ProtocolMessage;

    fn new_handler(&mut self) -> Self::ProtocolsHandler {
        IntoProtocolsHandler::select(Default::default(), self.ping.new_handler())
    }

    fn addresses_of_peer(&mut self, _: &PeerId) -> Vec<Multiaddr> {
        vec![]
    }

    fn inject_connected(&mut self, _: &PeerId) {}

    fn inject_disconnected(&mut self, _: &PeerId) {}

    fn inject_event(
        &mut self,
        peer_id: PeerId,
        cid: ConnectionId,
        event: EitherOutput<ProtocolMessage, PingResult>,
    ) {
        match event {
            EitherOutput::First(pm) => {
                log::debug!(
                    "Client received event from {}: {:?}",
                    peer_id.to_base58(),
                    &pm
                );
                // TODO: return "from" peer_id in GenerateEvent?
                self.events
                    .push_back(NetworkBehaviourAction::GenerateEvent(pm))
            }
            EitherOutput::Second(ping) => self.ping.inject_event(peer_id, cid, ping),
        }
    }

    fn poll(&mut self, cx: &mut Context, params: &mut impl PollParameters) -> Poll<SwarmEventType> {
        // just polling it to the end
        while let Poll::Ready(_) = self.ping.poll(cx, params) {}

        if let Some(event) = self.events.pop_front() {
            return Poll::Ready(event);
        }

        Poll::Pending
    }
}
