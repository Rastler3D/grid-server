use std::error::Error;
use std::io::ErrorKind;
use std::net::{SocketAddr, ToSocketAddrs};
use std::time::Duration;
use clap::{Parser, ValueEnum};

#[derive(Parser,Debug)]
#[command(version, about)]
pub struct Config{
    #[arg(long = "bind", short = 'b', env = "BIND_ADDRESS", value_parser = parse_addr, help = "Bind address of the Node Manager")]
    pub bind_address: Option<SocketAddr>,
    #[arg(env = "SERVER_ADDRESS", value_parser = parse_addr, help = "Optional address of the Node Manager")]
    pub server_addr: Option<SocketAddr>,
    #[arg(long = "library", short = 'l', env = "LIBRARY_TYPE", value_enum, default_value = "dll", help = "Library type")]
    pub library_type: LibraryType,
    #[arg(long = "name", short = 'n', env = "NAME",  help = "Optional name")]
    pub name: Option<String>,
    #[arg(long = "heartbeat", short = 'i', env = "HEARTBEAT_INTERVAL", default_value = "300 ms", value_parser = humantime::parse_duration, help = "Heartbeat interval")]
    pub heartbeat_interval: Duration
}

#[derive(ValueEnum,Debug, Clone, Copy)]
pub enum LibraryType{
    #[value(help = "Use Wasm runtime for running jobs")]
    Wasm,
    #[value(help = "Use Dynamic library loading for running jobs")]
    Dll
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