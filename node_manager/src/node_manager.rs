
use tracing::instrument;
use quinn::{Connecting, Connection, Endpoint};
use std::collections::HashMap;
use std::error::Error;
use std::future::{Future, pending};
use std::io;
use std::io::ErrorKind;
use std::marker::Tuple;
use std::mem::{replace, take};

use crate::node::{Node, NodeEvent, NodeSink};
use futures_util::{SinkExt, Stream, StreamExt, TryStreamExt};
use rustls::Certificate;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use mdns_sd::ServiceDaemon;
use shared::certificate::{ConfigError, ServiceType};
use shared::node::{NodeRequest, NodeResponse};
use shared::node_manager::manager_discovery::NodeManagerDiscoveryService;
use shared::node_manager::{NodeManagerRequest, NodeManagerResponse, WorkerInfo};
use shared::utils::bit_set::BitSet;
use shared::utils::linked_vec_map::LinkedVecMap;
use shared::utils::linked_vec_set::LinkedVecSet;
use shared::utils::channel::{Return, stream_channel};
use shared::utils::vec_map::VecMap;
use shared::worker::WorkerResponse;
use slab::Slab;
use thiserror::Error;
use tokio::select;
use tokio::sync::{mpsc, Mutex, Notify, oneshot};
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::{CancellationToken, PollSender};
use tokio_util::time::delay_queue::{Expired, Key};
use tokio_util::time::DelayQueue;
use tracing::{debug, info};
use uuid::Uuid;
use crate::cli::Config;
use crate::command::Command;
use crate::connection::{accept_incoming, ConnectionError};
use crate::http_server::start_server;
use crate::scheduler::{Scheduler, SchedulerEvent, SchedulerSink, Set, WorkersDemand};
use crate::server_config::server_config::ServerConfigBuilder;


const DEFAULT_PORT: u16 = 7294;


pub struct NodeManager {
    commands: ReceiverStream<Command>,
    discovery_service: ServiceDaemon,
    schedulers: Slab<Scheduler>,
    workers_demands_queue:  LinkedVecMap<WorkersDemand>,
    unassigned_workers: LinkedVecSet,
    workers: Slab<Node> ,
    expirations: DelayQueue<(Id, ServiceType)>,
    scheduler_timeout: Duration,
    disconnected: HashMap<Uuid, (Id, Key)>,
    node_timeout: Duration,
    endpoint: Endpoint,
    need_process_queue: Notify
}

impl NodeManager {
    pub async fn new(config: Config) -> Result<Self, anyhow::Error> {
        let (sender,receiver) = channel(1024);
        let _ = start_server(config.bind_api_server_address, sender).await?;
        let uuid = Uuid::new_v4();
        let addr = config.bind_address.unwrap_or(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), DEFAULT_PORT));

        let endpoint_config = ServerConfigBuilder {
            certificate_provider: None,
            uuid: uuid,
            name: config.name.clone(),
        }
        .build()?;

        let server_name = config.name.unwrap_or_else(|| uuid.to_string());
        let endpoint = Endpoint::server(endpoint_config, addr)?;
        let discovery_service = NodeManagerDiscoveryService::start_service(&server_name, &uuid.to_string(), addr.port())?;

        Ok(NodeManager {
            discovery_service,
            endpoint,
            commands: ReceiverStream::new(receiver),
            node_timeout: config.node_timeout,
            schedulers: Slab::new(),
            workers_demands_queue: LinkedVecMap::new(),
            workers: Slab::new(),
            expirations: DelayQueue::new(),
            scheduler_timeout: config.scheduler_timeout,
            unassigned_workers: LinkedVecSet::new(),
            need_process_queue: Notify::new(),
            disconnected: Default::default(),
        })
    }

    pub async fn start(&mut self) {
        let (mut node_stream, node_sink) = stream_channel(1024);
        let (mut scheduler_stream, scheduler_sink) = stream_channel(1024);
        tokio::spawn(accept_incoming(self.endpoint.clone(), (scheduler_sink,node_sink)));

        loop {
            select! {
                Some(node_event) = node_stream.next() => self.process_node(node_event).await,
                Some(expired) = self.expirations.next() => self.process_expiration(expired).await,
                Some(scheduler_event) = scheduler_stream.next() => self.process_scheduler(scheduler_event).await,
                Some(command) = self.commands.next() => self.process_command(command).await,
                _ = self.need_process_queue.notified() => self.process_queue().await
            }
        }
    }

    async fn process_command(&mut self, command: Command) {
        match command {
            Command::Nodes(returning) => {
                let nodes = self.workers.iter().map(|(_,node)| node.into()).collect();

                returning(nodes)
            },
            Command::Schedulers(returning) => {
                let schedulers = self.schedulers.iter().map(|(_,scheduler)| scheduler.into()).collect();

                returning(schedulers)
            },
            Command::Queue(returning) => {
                let queue = self.workers_demands_queue.iter().map(|x| x.1).cloned().collect();

                returning(queue)
            }
        }
    }

    #[instrument(skip_all, fields(service_type = ?expired.get_ref().1, id = %expired.get_ref().0, ))]
    async fn process_expiration(&mut self, expired: Expired<(Id,ServiceType)>){
        match expired.into_inner(){
            (id, ServiceType::Node) => {
                info!("Node heartbeat timeout");
                self.process_node_disconnection(id).await
            },
            (id, ServiceType::Scheduler) => {
                info!("Scheduler disconnect timeout");
                if let Some(worker) = self.workers.get_mut(id){
                    worker.cancellation_token.cancel();
                }
            }
            _ => { unreachable!() }
        }

    }

    async fn process_node_disconnection(&mut self, worker_id: Id){
        info!(%worker_id, "Node disconnected");
        if let Some(worker) = self.workers.try_remove(worker_id){
            self.disconnected.remove(&worker.uuid);
            match worker.assigned {
                None => { self.unassigned_workers.remove(worker_id); }
                Some(scheduler_id) => {
                    if let Some(scheduler) = self.schedulers.get_mut(scheduler_id){
                        if scheduler.assigned_workers.remove(worker_id){
                            self.workers_demands_queue.get_mut(scheduler_id).map(|x| x.provided_workers -= 1);
                            scheduler.send(NodeManagerResponse::Disconnect(worker_id)).await;
                            self.need_process_queue.notify_one();
                        }
                    }
                }
            }
        }
    }

    async fn process_scheduler_disconnection(&mut self, scheduler_id: Id){
        info!(%scheduler_id, "Scheduler disconnected");
        if let Some(mut scheduler) = self.schedulers.try_remove(scheduler_id){
            if let Some(_) = self.workers_demands_queue.remove(scheduler_id) {
                self.release_workers(&scheduler.assigned_workers).await;
            }

            self.disconnected.remove(&scheduler.uuid);
            self.need_process_queue.notify_one();
        }
    }

    async fn restore_node_session(&mut self, node_id: Id, key: Key, sink: NodeSink, cancellation_token: CancellationToken, returning: Return<Id>){
        let node = &mut self.workers[node_id];
        self.expirations.reset(&key,self.node_timeout);
        let buffer = node.disconnect_buffer.take().unwrap_or(Vec::new());
        node.cancellation_token = cancellation_token;
        node.sink = sink;

        for response in buffer{
            node.send(response).await;
        }

        returning(node_id);
    }

    async fn restore_scheduler_session(&mut self, scheduler_id: Id, key: Key, sink: SchedulerSink, cancellation_token: CancellationToken, returning: Return<Id>){
        let scheduler = &mut self.schedulers[scheduler_id];
        self.expirations.remove(&key);
        let buffer = scheduler.disconnect_buffer.take().unwrap_or(Vec::new());
        scheduler.cancellation_token = cancellation_token;
        scheduler.sink = sink;
        scheduler.send(NodeManagerResponse::RestoredSession).await;
        for response in buffer{
            scheduler.send(response).await;
        }

        returning(scheduler_id);
    }

    #[instrument(skip_all)]
    async fn process_scheduler(&mut self, scheduler_event: SchedulerEvent){
        match scheduler_event {
            SchedulerEvent::Connect { uuid, name, address, cancellation_token, sink, returning } => {
                if let Some((id,key)) = self.disconnected.remove(&uuid){
                    info!(%uuid, ?name, %address, "Restoring scheduler session");
                    self.restore_scheduler_session(id, key, sink, cancellation_token, returning).await;
                } else {
                    info!(%uuid, ?name, %address, "Scheduler connected");
                    let mut scheduler = Scheduler {
                        assigned_workers: LinkedVecSet::new(),
                        uuid,
                        name,
                        address,
                        cancellation_token,
                        sink,
                        disconnect_buffer: None,
                    };
                    scheduler.send(NodeManagerResponse::CreatedSession).await;
                    let id = self.schedulers.insert(scheduler);

                    returning(id);
                }
            },
            SchedulerEvent::Message(id, NodeManagerRequest::HelloMessage) => {},
            SchedulerEvent::Message(id, NodeManagerRequest::WorkersDemand { supported_platforms, required_workers }) => {
                info!(%id, ?supported_platforms, ?required_workers, "Scheduler requested workers");
                if let Some(scheduler) = self.schedulers.get_mut(id) {
                    if let Some(_) = self.workers_demands_queue.insert(id, WorkersDemand {
                        provided_workers: 0,
                        required_workers,
                        supported_platforms,
                    }) {
                        let assigned_workers = take(&mut scheduler.assigned_workers);
                        self.release_workers(assigned_workers).await;
                    }

                    self.need_process_queue.notify_one();
                }
            },
            SchedulerEvent::Message(id, NodeManagerRequest::ReleaseDemand) => {
                info!(%id, "Scheduler released workers");
                if let Some(scheduler) = self.schedulers.get_mut(id) {
                    if let Some(_) = self.workers_demands_queue.remove(id) {
                        let assigned_workers = take(&mut scheduler.assigned_workers);
                        self.release_workers(assigned_workers).await;
                    }

                    self.need_process_queue.notify_one();
                }
            }
            SchedulerEvent::Disconnect(id) => {
                if let Some(scheduler) = self.schedulers.get_mut(id){
                    let key = self.expirations.insert((id, ServiceType::Scheduler), self.scheduler_timeout);
                    self.disconnected.insert(scheduler.uuid, (id,key));
                    scheduler.disconnect_buffer = Some(Vec::new());
                }
            }
        }
    }

    async fn process_queue(&mut self){
        info!("Processing queue");
        let mut curr_worker = self.unassigned_workers.front().and_then(|x| self.workers.get_mut(x).map(|y| (x,y)));
        let mut map = HashMap::new();
        while let Some((worker_id, worker)) = curr_worker.take() {
            'scheduler: for (&scheduler_id, demand) in &mut self.workers_demands_queue {
                if let Some(required) = demand.required_workers {
                    if required == demand.provided_workers {
                        continue 'scheduler;
                    }
                }
                if demand.supported_platforms.contains(&worker.platform) {
                    demand.provided_workers += 1;
                    map.entry(scheduler_id).or_insert(Vec::new()).push(worker_id);
                    self.unassigned_workers.pop_front();
                    curr_worker = self.unassigned_workers.front().and_then(|x| self.workers.get_mut(x).map(|y| (x,y)));
                    break 'scheduler;
                }
            }
        }

        for (scheduler_id, workers_id) in map{
            let scheduler = &mut self.schedulers[scheduler_id];
            let mut workers = Vec::with_capacity(workers_id.len());
            for worker_id in workers_id{
                let worker = &mut self.workers[worker_id];
                worker.assigned = Some(scheduler_id);
                scheduler.assigned_workers.insert(worker_id);
                workers.push(WorkerInfo{
                    worker_id,
                    address: worker.address,
                    platform: worker.platform,
                    subworkers: worker.subworkers,
                });
                worker.send(NodeRequest::AssignClient(scheduler.address)).await;
            }

            scheduler.send(NodeManagerResponse::Connect(workers)).await;
        }
    }

    #[instrument(skip_all)]
    async fn process_node(&mut self, event: NodeEvent){
        match event {
            NodeEvent::Connect{
                uuid, name, platform,
                subworkers, address, cancellation_token,
                sink, returning
            } => {
                if let Some((id,key)) = self.disconnected.remove(&uuid){
                    info!(%uuid, ?name, ?platform, %subworkers, %address, "Restoring node session");
                    self.restore_node_session(id, key, sink, cancellation_token, returning).await;
                } else {
                    info!(%uuid, ?name, ?platform, %subworkers, %address, "Node connected");
                    let vacant_entry = self.workers.vacant_entry();
                    let worker_id = vacant_entry.key();
                    let expiration_key = self.expirations.insert((worker_id, ServiceType::Node), self.node_timeout);
                    let node = Node {
                        assigned: None,
                        uuid,
                        name,
                        platform,
                        subworkers,
                        address,
                        expiration_key,
                        cancellation_token,
                        sink,
                        disconnect_buffer: None,
                    };
                    vacant_entry.insert(node);

                    returning(worker_id);

                    self.unassigned_workers.insert(worker_id);
                    self.need_process_queue.notify_one();
                }
            }
            NodeEvent::Disconnect(id) => {
                if let Some(node) = self.workers.get_mut(id){
                    self.disconnected.insert(node.uuid, (id, node.expiration_key));
                    node.disconnect_buffer = Some(Vec::new());
                }
            }
            NodeEvent::Message(worker_id, NodeResponse::Heartbeat) => {
                debug!(%worker_id, "Node sent heartbeat");
                if let Some(node) = self.workers.get_mut(worker_id){
                    self.expirations.reset(&node.expiration_key, self.node_timeout);
                }
            },
        }
    }

    async fn release_workers(&mut self, workers_id: impl IntoIterator<Item = Id>){
        for worker_id in workers_id{
            self.release_worker(worker_id).await;
        }
    }

    async fn release_worker(&mut self, worker_id: Id){
        info!(%worker_id, "Releasing worker");
        if let Some(worker) = self.workers.get_mut(worker_id){
            self.unassigned_workers.insert(worker_id);
            worker.assigned = None;
            worker.send(NodeRequest::UnassignClient).await;
        }
    }
}




pub type Id = usize;

