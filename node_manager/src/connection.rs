use std::future::Future;
use crate::node::{Node, NodeEvent};
use crate::node_manager::{ Id};
use futures_util::{SinkExt, StreamExt};
use quinn::{Connecting, Connection, Endpoint};
use rustls::Certificate;
use shared::certificate::{ServiceType, PLATFORM_OID, SERVICE_TYPE_OID, SUBWORKERS_OID, ConfigError};
use shared::worker::{WorkerRequest, WorkerResponse};
use std::io;
use std::io::{Error, ErrorKind};
use std::str::FromStr;
use std::time::Duration;
use futures_sink::Sink;
use shared::node::{NodeRequest, NodeResponse};
use shared::node_manager::{NodeManagerRequest, NodeManagerResponse};
use shared::platform::Platform;
use shared::scheduler::SchedulerResponse;
use shared::utils::bin_codec::BinCodec;
use shared::utils::channel::return_channel;
use slab::Slab;
use thiserror::Error;
use tokio::select;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio_util::codec::{FramedRead, FramedWrite};
use tokio_util::sync::{CancellationToken, PollSender};
use tracing::{error, Instrument};
use uuid::Uuid;
use x509_parser::extensions::GeneralName::DNSName;
use x509_parser::oid_registry::Oid;
use x509_parser::prelude::{AttributeTypeAndValue, FromDer, X509Certificate};
use crate::scheduler::SchedulerEvent;
use tracing::instrument;
use tracing::info;

pub async fn accept_incoming(endpoint: Endpoint, event_sink: (PollSender<SchedulerEvent>, PollSender<NodeEvent>)) {
    while let Some(conn) = endpoint.accept().await {
        tokio::spawn(handle_connection(conn, event_sink.clone()));
    }
}

#[instrument(skip_all, fields(address = %connecting.remote_address()))]
async fn handle_connection(connecting: Connecting, (mut scheduler_sink, mut node_sink): (PollSender<SchedulerEvent>, PollSender<NodeEvent>)) {
    let a: Result<_, _> = try {
        let connection = connecting.await?;
        let certificate = connection
                .peer_identity()
                .and_then(|x| x.downcast::<Vec<Certificate>>().ok())
                .and_then(|mut x| x.first().cloned())
                .ok_or(io::Error::from(ErrorKind::PermissionDenied))?;
        let (_, certificate) = X509Certificate::from_der(certificate.as_ref())
            .map_err(|err| io::Error::new(ErrorKind::InvalidData, err))?;
        let service_type = parse_service_type(&certificate)?;
        info!(?service_type, "Incoming connection.");
        match service_type {
            Some(ServiceType::Node) => {
                handle_node(connection, certificate, node_sink).await?
            },
            Some(ServiceType::Scheduler) => {
                handle_scheduler(connection, certificate, scheduler_sink).await?
            },
            _ => Err(io::Error::from(ErrorKind::Unsupported))?,
        }
    };
    let _a = match a {
        Ok(a) => a,
        Err::<_, anyhow::Error>(err) => {
            error!(%err, "Connection error");
            return;
        }
    };
}

async fn handle_scheduler(connection: Connection, certificate: X509Certificate<'_>, mut event_sink: PollSender<SchedulerEvent>) -> Result<(), anyhow::Error>{
    let (write, read) = connection.accept_bi().await?;

    let stream = FramedRead::new(read, BinCodec::<NodeManagerRequest>::new());
    let sink = FramedWrite::new(write, BinCodec::<NodeManagerResponse>::new());
    let address = connection.remote_address();
    let cancellation_token = CancellationToken::new();
    let (uuid, name) = parse_scheduler(&certificate)?;
    let (id, returning) = return_channel();
    let mut event_sink = event_sink.sink_map_err(|x| io::Error::from(ErrorKind::BrokenPipe));

    event_sink.send(SchedulerEvent::Connect{
        uuid,
        name,
        address,
        sink,
        returning,
        cancellation_token: cancellation_token.clone(),
    }).await?;
    let id =id.await?;

    let mut forward = stream
        .map(|x| x.map(|x| SchedulerEvent::Message(id, x)))
        .forward(&mut event_sink);

    select! {
        _ = forward => {
            event_sink.send(SchedulerEvent::Disconnect(id)).await?;

            Err(Error::from(ErrorKind::ConnectionRefused))?
        },
        _ = cancellation_token.cancelled() => {
            //event_sink.send(SchedulerEvent::Disconnect(id)).await?;
        }
    }
    Ok(())
}

async fn handle_node(connection: Connection, certificate: X509Certificate<'_>, mut event_sink: PollSender<NodeEvent>) -> Result<(), anyhow::Error>{
    let (write, read) = connection.accept_bi().await?;
    let stream = FramedRead::new(read, BinCodec::<NodeResponse>::new());
    let sink = FramedWrite::new(write, BinCodec::<NodeRequest>::new());
    let address = connection.remote_address();
    let cancellation_token = CancellationToken::new();
    let (uuid,platform,subworkers,name) = parse_node(&certificate)?;
    let (id, returning) = return_channel();
    let mut event_sink = event_sink.sink_map_err(|x| io::Error::from(ErrorKind::BrokenPipe));
    event_sink.send(NodeEvent::Connect{
        uuid,
        name,
        platform,
        subworkers,
        address,
        sink,
        returning,
        cancellation_token: cancellation_token.clone(),
    }).await?;
    let id =id.await?;

    let mut forward = stream
        .map(|x| x.map(|x| NodeEvent::Message(id, x)))
        .forward(&mut event_sink);

    select! {
        _ = forward => {
            event_sink.send(NodeEvent::Disconnect(id)).await?;

            Err(Error::from(ErrorKind::ConnectionRefused))?
        },
        _ = cancellation_token.cancelled() => {
            event_sink.send(NodeEvent::Disconnect(id)).await?;
        }
    }
    Ok(())
}



fn parse_service_type(certificate: &X509Certificate) -> io::Result<Option<ServiceType>>{
    let service_type_oid = Oid::from(SERVICE_TYPE_OID).unwrap();
    let service_type = certificate
        .tbs_certificate
        .find_extension(&service_type_oid)
        .and_then(|x| {
            bincode::serde::decode_from_slice::<ServiceType, _>(
                x.value,
                bincode::config::standard(),
            )
                .ok()
        })
        .map(|x| x.0);

    Ok(service_type)
}

fn parse_node(certificate: &X509Certificate) -> io::Result<(Uuid, Platform, usize, Option<String>)>{
    let subworkers_oid = Oid::from(SUBWORKERS_OID).unwrap();
    let platform_oid = Oid::from(PLATFORM_OID).unwrap();
    let subworkers = certificate.tbs_certificate.find_extension(&subworkers_oid)
        .and_then(|x| x.value.try_into().map(usize::from_be_bytes).ok())
        .unwrap_or(1);
    let platform = certificate.tbs_certificate.find_extension(&platform_oid)
        .and_then(|x| bincode::serde::decode_from_slice::<Platform, _>(x.value, bincode::config::standard()).ok())
        .map(|x| x.0)
        .ok_or(io::Error::from(ErrorKind::NotFound))?;
    let uuid = certificate
        .subject()
        .iter_common_name()
        .map(AttributeTypeAndValue::as_str)
        .flatten()
        .map(Uuid::from_str)
        .flatten()
        .next()
        .ok_or(io::Error::from(ErrorKind::NotFound))?;
    let name = certificate
        .tbs_certificate
        .subject_alternative_name()
        .iter()
        .flat_map(|(_,x)| &x.general_names)
        .map(|x| if let DNSName(name) = x { Some(name.to_string()) } else { None })
        .flatten()
        .next();

    Ok((uuid, platform, subworkers, name))
}

fn parse_scheduler(certificate: &X509Certificate) -> io::Result<(Uuid, Option<String>)>{
    let uuid = certificate
        .subject()
        .iter_common_name()
        .map(AttributeTypeAndValue::as_str)
        .flatten()
        .map(Uuid::from_str)
        .flatten()
        .next()
        .ok_or(io::Error::from(ErrorKind::NotFound))?;
    let name = certificate
        .tbs_certificate
        .subject_alternative_name()
        .iter()
        .flat_map(|(_,x)| &x.general_names)
        .map(|x| if let DNSName(name) = x { Some(name.to_string()) } else { None })
        .flatten()
        .next();

    Ok((uuid, name))
}


#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Quic(#[from] quinn::ConnectionError),
    #[error(transparent)]
    Mdns(#[from] mdns_sd::Error)
}

