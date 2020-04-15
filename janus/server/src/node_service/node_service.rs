/*
 * Copyright 2020 Fluence Labs Limited
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

use crate::config::NodeServiceConfig;
use crate::node_service::p2p::P2PBehaviour;
use janus_libp2p::{build_transport, types::OneshotOutlet};

use async_std::task;
use futures::channel::oneshot::Receiver;
use futures::{channel::oneshot, select, stream::StreamExt, FutureExt};
use futures_util::future::IntoStream;
use futures_util::stream::Fuse;
use libp2p::{
    identity::ed25519::{self, Keypair},
    identity::PublicKey,
    PeerId, Swarm, TransportError,
};
use log::error;
use parity_multiaddr::{Multiaddr, Protocol};
use std::io;

type NodeServiceSwarm = Swarm<P2PBehaviour>;

/// Responsibilities:
/// - Command swarm to listen for other nodes
/// - Handle events from peers and send them to swarm
/// - Proxy events from swarm to peer service
pub struct NodeService {
    swarm: NodeServiceSwarm,
    config: NodeServiceConfig,
}

impl NodeService {
    pub fn new(
        key_pair: Keypair,
        config: NodeServiceConfig,
        root_weights: Vec<(ed25519::PublicKey, u32)>,
    ) -> Box<Self> {
        let NodeServiceConfig { socket_timeout, .. } = config;

        let local_peer_id = PeerId::from(PublicKey::Ed25519(key_pair.public()));
        println!("node service is starting with id = {}", local_peer_id);

        let mut swarm = {
            let behaviour =
                P2PBehaviour::new(key_pair.clone(), local_peer_id.clone(), root_weights);
            let key_pair = libp2p::identity::Keypair::Ed25519(key_pair);
            let transport = build_transport(key_pair, socket_timeout);

            Swarm::new(transport, behaviour, local_peer_id)
        };

        if let Some(external_address) = config.external_address {
            let external_tcp = {
                let mut maddr = Multiaddr::from(external_address);
                maddr.push(Protocol::Tcp(config.listen_port));
                maddr
            };

            let external_ws = {
                let mut maddr = Multiaddr::from(external_address);
                maddr.push(Protocol::Tcp(config.websocket_port));
                maddr.push(Protocol::Ws("/".into()));
                maddr
            };

            Swarm::add_external_address(&mut swarm, external_tcp);
            Swarm::add_external_address(&mut swarm, external_ws);
        }

        let node_service = Self { swarm, config };

        Box::new(node_service)
    }

    /// Starts node service
    pub fn start(mut self: Box<Self>) -> OneshotOutlet<()> {
        let (exit_sender, exit_receiver) = oneshot::channel();
        let mut exit_receiver: Fuse<IntoStream<Receiver<()>>> = exit_receiver.into_stream().fuse();

        self.listen().expect("Error on starting node listener");
        self.bootstrap();

        task::spawn(async move {
            loop {
                select!(
                    _ = self.swarm.select_next_some() => {},
                    _ = exit_receiver.next() => {
                        break
                    }
                )
            }
        });

        exit_sender
    }

    /// Starts node service listener.
    #[inline]
    fn listen(&mut self) -> Result<(), TransportError<io::Error>> {
        let mut listen_addr = Multiaddr::from(self.config.listen_ip);
        listen_addr.push(Protocol::Tcp(self.config.listen_port));

        let mut ws = Multiaddr::from(self.config.listen_ip);
        ws.push(Protocol::Tcp(self.config.websocket_port));
        ws.push(Protocol::Ws("/".into()));

        Swarm::listen_on(&mut self.swarm, listen_addr)?;
        Swarm::listen_on(&mut self.swarm, ws)?;
        Ok(())
    }

    /// Dials bootstrap nodes, and then commands swarm to bootstrap itself.
    #[inline]
    fn bootstrap(&mut self) {
        for addr in &self.config.bootstrap_nodes {
            let dial_result = Swarm::dial_addr(&mut self.swarm, addr.clone());

            if let Err(err) = dial_result {
                error!("Error dialing {}: {}", addr, err)
            }
        }

        if self.config.bootstrap_nodes.is_empty() {
            log::info!("No bootstrap nodes found. Am I the only one? :(");
        }

        self.swarm.bootstrap();
    }
}
