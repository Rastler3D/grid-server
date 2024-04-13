use std::net::{SocketAddr, ToSocketAddrs};
use std::time::Duration;
use clap::{Parser, ValueEnum};

#[derive(Parser,Debug)]
#[command(version, about)]
pub struct Config{
    #[arg(long = "bind", short = 'b', env = "BIND_ADDRESS", value_parser = parse_addr, help = "Bind address of the Node Manager")]
    pub bind_address: Option<SocketAddr>,
    #[arg(long = "timeout", short = 't', env = "NODE_TIMEOUT", default_value = "5 min", value_parser = humantime::parse_duration, help = "Node heartbeat timeout")]
    pub node_timeout: Duration,
    #[arg(long = "scheduler_timeout", short = 's', env = "SCHEDULER_TIMEOUT", default_value = "15 min", value_parser = humantime::parse_duration, help = "Scheduler disconnect timeout")]
    pub scheduler_timeout: Duration,
    #[arg(long = "api-server", short = 'a', env = "BIND_API_SERVER_ADDRESS", value_parser = parse_addr, help = "Bind address of the Task manager api-server")]
    pub bind_api_server_address: Option<SocketAddr>,
    #[arg(long = "name", short = 'n', env = "NAME",  help = "Optional name")]
    pub name: Option<String>,
}


fn parse_addr(addr: &str) -> Result<SocketAddr,String> {
    match addr.to_socket_addrs() {
        Ok(addr_iter) => {
            if let Some(addr) = addr_iter.filter(|x| x.is_ipv4()).next() {
                Ok(addr)
            } else {
                Err("Host name resolved to zero ipV4-addresses".to_string())
            }
        }
        Err(err) => {
            Err(err.to_string())
        }
    }
}