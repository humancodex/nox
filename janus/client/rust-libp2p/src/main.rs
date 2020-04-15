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

#![recursion_limit = "512"]
#![deny(
    dead_code,
    nonstandard_style,
    unused_imports,
    unused_mut,
    unused_variables,
    unused_unsafe,
    unreachable_patterns
)]

use janus_client::client::Client;

use async_std::{io, task};
use faas_api::{Address, FunctionCall};
use futures::prelude::*;
use futures::{select, stream::StreamExt};
use janus_client::Command;
use libp2p::PeerId;

use ctrlc_adapter::block_until_ctrlc;
use futures::channel::oneshot;
use parity_multiaddr::Multiaddr;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let relay_addr: Multiaddr = std::env::args()
        .nth(1)
        .expect("multiaddr of relay peer should be provided by the first argument")
        .parse()
        .expect("provided wrong  Multiaddr");

    let bootstrap_id: PeerId = std::env::args()
        .nth(2)
        .expect("peer id should be provided by the second argument")
        .parse()
        .expect("provided wrong PeerId");

    let (exit_sender, exit_receiver) = oneshot::channel::<()>();

    let client_task = task::spawn(async move {
        run_client(exit_receiver, bootstrap_id, relay_addr)
            .await
            .expect("Error running client"); // TODO: handle errors
    });

    block_until_ctrlc();
    exit_sender.send(()).unwrap();
    task::block_on(client_task);

    Ok(())
}

async fn run_client(
    exit_receiver: oneshot::Receiver<()>,
    bootstrap_id: PeerId,
    relay: Multiaddr,
) -> Result<(), Box<dyn Error>> {
    let client = Client::connect(relay, exit_receiver);
    let (mut client, client_task) = client.await?;
    print_example(&client.peer_id, bootstrap_id);

    let mut stdin = io::BufReader::new(io::stdin()).lines().fuse();

    loop {
        select!(
            from_stdin = stdin.select_next_some() => {
                match from_stdin {
                    Ok(line) => {
                        let cmd: Result<Command, _> = serde_json::from_str(&line);
                        if let Ok(cmd) = cmd {
                            client.send(cmd);
                        } else {
                            println!("incorrect string provided");
                        }
                    }
                    Err(_) => panic!("Stdin closed"),
                }
            },
            incoming = client.receive_one() => {
                match incoming {
                    Some(msg) => println!("Received\n{}", serde_json::to_string_pretty(&msg).unwrap()),
                    None => {
                        println!("Client closed");
                        break;
                    }
                }
            }
        )
    }

    client_task.await;

    Ok(())
}

fn print_example(peer_id: &PeerId, bootstrap: PeerId) {
    use serde_json::json;
    use std::time::SystemTime;
    fn show(cmd: Command) {
        println!("{}", serde_json::to_value(cmd).unwrap());
    }

    let time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string();
    let uuid = uuid::Uuid::new_v4().to_string();

    let call_example = Command::Call {
        node: bootstrap.clone(),
        call: FunctionCall {
            uuid,
            target: Some(Address::Service {
                service_id: "IPFS.multiaddr".into(),
            }),
            reply_to: Some(Address::Relay {
                relay: bootstrap,
                client: peer_id.clone(),
            }),
            arguments: json!({ "hash": "QmFile", "msg_id": time }),
            name: Some("name!".to_string()),
        },
    };

    println!("possible messages:");
    show(call_example);
    println!("\n")
}
