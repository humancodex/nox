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

#[allow(clippy::module_inception)]
mod node_service;
mod p2p {
    mod behaviour;

    pub use behaviour::P2PBehaviour;
}

pub mod function {
    mod builtin_service;
    mod provider;
    mod router;

    pub use provider::Provider;
    pub use router::FunctionRouter;
    pub(crate) use router::SwarmEventType;
}

pub use node_service::NodeService;
pub use p2p::P2PBehaviour;
