use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::net::SocketAddr;
use std::ops::{DerefMut, Deref};
use async_stream::try_stream;
use tokio_stream::Stream;

pub const NODE_MANAGER_SERVICE_TYPE: &'static str = "_node-manager._udp.local.";
pub struct NodeManagerDiscoveryService;


impl NodeManagerDiscoveryService {
    pub fn discover() -> impl Stream<Item = Result<SocketAddr, mdns_sd::Error>> {
        try_stream! {
            let daemon = DropableDaemon(ServiceDaemon::new()?);
            let events = daemon.browse(NODE_MANAGER_SERVICE_TYPE)?.into_stream();
            for await event in events {
                match event{
                    ServiceEvent::ServiceResolved(server) => {
                        let port = server.get_port();
                        for &addr in server.get_addresses().iter().filter(|x| x.is_ipv4()) {

                            yield SocketAddr::new(addr.clone(), port);
                        }
                    }
                    _ => continue,
                }
            }
        }
    }

    pub fn start_service(name: &str, hostname: &str, port: u16) -> Result<ServiceDaemon, mdns_sd::Error> {
        let daemon = ServiceDaemon::new()?;
        let service_info = ServiceInfo::new(NODE_MANAGER_SERVICE_TYPE, name, hostname, (), port, None)?.enable_addr_auto();

        daemon.register(service_info)?;

        Ok(daemon)
    }
}


struct DropableDaemon(ServiceDaemon);

impl Drop for DropableDaemon {
    fn drop(&mut self) {
        self.0.shutdown().map(|x| x.recv());
    }
}
impl Deref for DropableDaemon {
    type Target = ServiceDaemon;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for DropableDaemon {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}