use anyhow::Ok;
use async_trait::async_trait;
use futures::StreamExt;
use libp2p::{Multiaddr, PeerId, mdns, swarm::SwarmEvent};
use tokio::sync::{Mutex, mpsc::{Receiver, Sender}};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;
use crate::{network::{MyBehaviour, MyBehaviourEvent}, recorder::RawFrame, service::{Client, Server}};

#[cfg(target_os = "linux")]
use crate::recorder::RecorderLinux;

#[cfg(target_os = "windows")]
pub type ScreenRecorder<'a> = RecorderWindows<'a>;

#[cfg(target_os = "linux")]
pub type ScreenRecorder<'a> = RecorderLinux<'a>; 

pub struct App {
    pub uuid: Uuid,
    pub username: String,
    pub screen_recorder: ScreenRecorder<'static>,
    pub discovered_peers: Mutex<Vec<(PeerId, Multiaddr)>>,
    pub connected_peers: Vec<(PeerId, Multiaddr)>,
    pub stream_sender: Sender<RawFrame>,
    pub encoder_receiver: Mutex<Receiver<Vec<u8>>>,
}

impl App {
    pub async fn new(stream_sender: Sender<RawFrame>, encoder_receiver: Receiver<Vec<u8>>) -> anyhow::Result<Self> {        
        Ok(Self {
            uuid: Uuid::new_v4(),
            username: "enter username".to_owned(),
            screen_recorder: ScreenRecorder::new().await?,
            discovered_peers: Mutex::new(vec![]),
            connected_peers: vec![],
            stream_sender,
            encoder_receiver: Mutex::new(encoder_receiver),
        })
    }
}

#[async_trait]
impl Client for App {
    async fn start_recording(&self) -> anyhow::Result<()> {
        self.screen_recorder.start_recording(self.stream_sender.clone()).await?;
        Ok(())
    }
    
    async fn draw_client(&self) -> anyhow::Result<()> {
        todo!()
    }
}

#[async_trait]
impl Server for App {
    async fn discover_peers(&self) -> anyhow::Result<()>{
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
                SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(event)) => if let mdns::Event::Discovered(peers) = event {
                    for (peer_id, addr) in peers {
                        println!("Discovered peer {} at {}", peer_id, addr);

                        self.discovered_peers.lock().await.push((peer_id, addr));
                        // Dial only when you want to watch them
                        // if want_to_watch_stream(&peer_id) {
                        //     swarm.dial(addr.clone())?;
                        // }
                    }
                }
                SwarmEvent::ConnectionEstablished { peer_id, .. } => { 
                    tracing::info!("Connected to peer {}", peer_id); 
                }
                _ => {}
            }
        }
    }

    async fn start_streaming(&self) -> anyhow::Result<()> {
        todo!()
    }
}