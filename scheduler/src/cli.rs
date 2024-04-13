use std::net::{SocketAddr, ToSocketAddrs};
use clap::Parser;

#[derive(Parser,Debug)]
#[command(version, about)]
pub struct Config{
    #[arg(long = "failed_executions", short = 'f', env = "FAILED_EXECUTIONS_LIMIT", default_value = "20",  help = "Limit of the failed executions")]
    pub(crate) failed_executions_limit: u16,
    #[arg(env = "NODE_MANAGER_ADDRESS", value_parser = parse_addr, help = "Address of the Node Manager")]
    pub(crate) node_manager_address: SocketAddr,
    #[arg(long = "bind", short = 'b', env = "BIND_ADDRESS", value_parser = parse_addr, help = "Bind address of the Scheduler")]
    pub bind_address: SocketAddr,
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