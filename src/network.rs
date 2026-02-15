use std::{collections::HashMap, io, time::Duration};

use futures::{AsyncReadExt, AsyncWriteExt, StreamExt};
use libp2p::{Multiaddr, PeerId, Stream, StreamProtocol, identify, mdns::Event, multiaddr::Protocol, ping, rendezvous, swarm::{NetworkBehaviour, SwarmEvent}};
use libp2p_stream::{self as stream, Control};
use libp2p::mdns;
// use rand::RngCore;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

const ECHO_PROTOCOL: StreamProtocol = StreamProtocol::new("/streaming");

struct StreamMetadata {
    name: String,
    peer_id: PeerId
}

struct Streamer {
    connected_peers: HashMap<PeerId, Control>,
}

struct Viewer {
    discovered_streams: HashMap<PeerId, StreamMetadata>,
}

#[derive(NetworkBehaviour)]
struct MyBehaviour {
    mdns: mdns::tokio::Behaviour,
}

impl MyBehaviour {
    fn new(mdns: mdns::tokio::Behaviour) -> Self {
        Self {
            mdns,
        }
    }
}

pub async fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env()?,
        )
        .init();

    let keypair = libp2p::identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(keypair.public());
    tracing::info!("Local PeerId: {}", peer_id);

    let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id)?;
    let behaviour = MyBehaviour::new(mdns);

    // Build swarm
    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_quic()
        .with_behaviour(|_| behaviour)?
        .build();

    swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;
    
    loop {
        match swarm.select_next_some().await {
            SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(event)) => match event {
                mdns::Event::Discovered(peers) => {
                    for (peer_id, addr) in peers {
                        println!("Discovered peer {} at {}", peer_id, addr);

                        // Dial only when you want to watch them
                        // if want_to_watch_stream(&peer_id) {
                        //     swarm.dial(addr.clone())?;
                        // }
                    }
                }
                _ => {}
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => { 
                tracing::info!("Connected to peer {}", peer_id); 
            }
            _ => {}
        }
    }
}