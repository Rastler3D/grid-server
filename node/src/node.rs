use std::io;
use std::io::ErrorKind;
use std::mem::take;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::pin::pin;
use std::time::Duration;
use anyhow::anyhow;
use async_stream::stream;
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use quinn::{Endpoint, RecvStream, SendStream};
use shared::node::{NodeRequest, NodeResponse};
use shared::node_manager::{NodeManagerRequest, NodeManagerResponse};
use shared::platform::{CURRENT_PLATFORM, WASM_PLATFORM};
use shared::utils::bin_codec::BinCodec;
use shared::utils::retry::establish_connection;
use tokio::select;
use tokio::time::interval;
use tokio_stream::{Stream};
use tokio_stream::wrappers::{IntervalStream, ReceiverStream};
use tokio_util::codec::{FramedRead, FramedWrite};
use tokio_util::sync::{CancellationToken, PollSender};
use tracing::{debug, error, info};
use shared::worker::WorkerResponse;
use crate::cli::{Config, LibraryType};
use crate::client::{Client, WorkerEvent};
use crate::endpoint_config::endpoint_config::EndpointConfigBuilder;
use crate::node_manager::{NodeManager, NodeManagerEvent};
use crate::dll::dll_worker::DllWorker;
use crate::wasm::wasm_worker::WasmWorker;
use crate::worker::Worker;



pub struct Node<W: Worker>{
    heartbeat: IntervalStream,
    config: Config,
    endpoint: Endpoint,
    node_manager: NodeManager,
    worker: W,
    client: Option<Client>
}

impl Node<WasmWorker>{
    pub async fn new_wasm(config: Config) -> Result<Node<WasmWorker>, anyhow::Error>{
        let (client_config, server_config) = EndpointConfigBuilder{
            name: config.name.clone(),
            platform: WASM_PLATFORM,
            certificate_provider: None
        }.build()?;

        let mut endpoint = Endpoint::server(server_config, config.bind_address.unwrap_or(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)))?;
        endpoint.set_default_client_config(client_config);
        let node_manager = NodeManager::connect(endpoint.clone(), config.server_addr);
        let worker = WasmWorker::new().await?;

        Ok(Self{
            heartbeat: IntervalStream::new(interval(config.heartbeat_interval)),
            config,
            node_manager,
            endpoint,
            worker,
            client: None
        })
    }
}

impl Node<DllWorker>{
    pub async fn new_dll(config: Config) -> Result<Node<DllWorker>, anyhow::Error>{
        let (client_config, server_config) = EndpointConfigBuilder{
            name: config.name.clone(),
            platform: CURRENT_PLATFORM,
            certificate_provider: None
        }.build()?;

        let mut endpoint = Endpoint::server(server_config, config.bind_address.unwrap_or(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)))?;
        endpoint.set_default_client_config(client_config);
        let node_manager = NodeManager::connect(endpoint.clone(), config.server_addr);
        let worker = DllWorker::new().await?;

        Ok(Self{
            heartbeat: IntervalStream::new(interval(config.heartbeat_interval)),
            config,
            node_manager,
            endpoint,
            worker,
            client: None
        })
    }
}
impl<W:Worker> Node<W>{
    pub async fn start(&mut self) -> Result<(), anyhow::Error>{
        loop {
            match self.client {
                None => {
                    select! {
                        Some(request) = self.node_manager.recv() => {
                            self.process_node_manager(request).await?;
                        },
                        Some(_) = self.heartbeat.next() => {
                            debug!("Sending heartbeat");
                            self.node_manager.send(NodeResponse::Heartbeat).await?;
                        },
                    }
                },
                Some(ref mut client) => {
                    select! {
                        Some(request) = self.node_manager.recv() => {
                            self.process_node_manager(request).await?;
                        },
                        Some(_) = self.heartbeat.next() => {
                            debug!("Sending heartbeat");
                            self.node_manager.send(NodeResponse::Heartbeat).await?;
                        },
                        Some(request) = client.recv() => {
                            let addr = client.addr;
                            self.process_request(addr, request).await?;
                        },
                        result = self.worker.recv() => {
                            client.send(result).await;
                        }
                    }

                }
            }
        }
    }

    async fn process_node_manager(&mut self, request: NodeManagerEvent) -> Result<(), anyhow::Error>{
        match request {
            NodeManagerEvent::Connected => {
                info!("Connected to node manager");
            }
            NodeManagerEvent::Disconnected => {
                info!("Node manager disconnected");
                self.node_manager = NodeManager::connect(self.endpoint.clone(), self.config.server_addr)
            }
            NodeManagerEvent::Message(message) => {
                match message {
                    NodeRequest::AssignClient(addr) => {
                        info!(%addr,"Node manager assign client");
                        self.worker.reset().await;
                        self.client = Some(Client::new(self.endpoint.clone(), addr));

                    }
                    NodeRequest::UnassignClient => {
                        info!("Node manager unassign client");
                        self.worker.reset().await;
                        self.client = None;
                    }
                }
            }
            NodeManagerEvent::ConnectionNotEstablished => {
                return Err(anyhow!("Failed to establish connection with node manager."));
            }
        }

        Ok(())
    }

    pub async fn process_request(&mut self, address: SocketAddr, request: WorkerEvent) -> Result<(), anyhow::Error>{
        match request{
            WorkerEvent::Connected => {
                info!(%address, "Connected client");
                if let Some(client) = &mut self.client{
                    client.connected = true;
                    for buffered in take(&mut client.buffer){
                        client.send(buffered).await;
                    }
                }
            }
            WorkerEvent::Disconnected => {
                info!(%address, "Disconnected client");
                if let Some(client) = &mut self.client{
                    client.connected = false;
                }
            }
            WorkerEvent::Message(request) => {
                debug!(%address, job_id = %request.job, tasks = %request.tasks.len(), replacements = %request.replacements.len(), with_job_data = %request.job_data.is_some() ,"Received client request");
                if let Err(response) = self.worker.process(request).await{
                    if let Some(client) = &mut self.client{
                        client.send(WorkerResponse::Error(response)).await?;
                    }
                }
            }
        }

        Ok(())
    }
}