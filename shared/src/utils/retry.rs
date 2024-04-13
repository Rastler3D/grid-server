use backoff::future::{retry, retry_notify};
use backoff::ExponentialBackoff;
use humantime::format_duration;
use quinn::{Connecting, Connection, ConnectionError, Endpoint, RecvStream, SendStream};
use std::net::SocketAddr;
use tracing::{error, info, instrument};

#[instrument(skip_all)]
pub async fn establish_connection(
    endpoint: &Endpoint,
    addr: SocketAddr,
    name: &str,
) -> Result<(SendStream, RecvStream), anyhow::Error> {
    info!(%addr, "Entablishing connection with {name}");
    let result = retry_notify(ExponentialBackoff::default(), || async {
        Ok(endpoint.connect(addr, "addr")
            .map_err(anyhow::Error::from)?
            .await
            .map_err(anyhow::Error::from)?
            .open_bi()
            .await
            .map_err(anyhow::Error::from)?)
    }, |error, duration| info!(%addr, %error,  "Failed to establish connection with  {name}. Retrying after {}.",format_duration(duration) )
    ).await;

    if result.is_err() {
        error!(%addr, "Unable to establish connection with {name}.");
    } else {
        info!(%addr, "Connection successfully established with {name}.");
    }

    result
}
