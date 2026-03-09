use std::collections::HashMap;

use libp2p::mdns;
use libp2p::{PeerId, StreamProtocol, swarm::NetworkBehaviour};
use libp2p_stream::Control;

const ECHO_PROTOCOL: StreamProtocol = StreamProtocol::new("/streaming");

struct StreamMetadata {
    name: String,
    peer_id: PeerId,
}

struct Streamer {
    connected_peers: HashMap<PeerId, Control>,
}

struct Viewer {
    discovered_streams: HashMap<PeerId, StreamMetadata>,
}

#[derive(NetworkBehaviour)]
pub struct MyBehaviour {
    mdns: mdns::tokio::Behaviour,
}

impl MyBehaviour {
    pub fn new(mdns: mdns::tokio::Behaviour) -> Self {
        Self { mdns }
    }
}
