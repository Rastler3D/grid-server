
use crate::server_config::certificate_provider::CertificateProvider;
use rcgen::RcgenError;
use std::sync::Arc;
use quinn::{ServerConfig, TransportConfig};

use rustls::server::AllowAnyAuthenticatedClient;
use shared::certificate::ConfigError;
use thiserror::Error;
use uuid::Uuid;

pub struct ServerConfigBuilder {
    pub(crate) certificate_provider: Option<CertificateProvider>,
    pub(crate) name: Option<String>,
    pub(crate) uuid: Uuid
}
impl ServerConfigBuilder {
    pub fn new(name: String) -> Self {
        Self {
            certificate_provider: None,
            uuid: Uuid::new_v4(),
            name: None,
        }
    }

    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn uuid(mut self, uuid: Uuid) -> Self {
        self.uuid = uuid;
        self
    }

    pub fn certificate_provider(mut self, provider: CertificateProvider) -> Self {
        self.certificate_provider = Some(provider);
        self
    }
    pub fn build(self) -> Result<quinn::ServerConfig, ConfigError> {
        let certificate_provider = self
            .certificate_provider
            .unwrap_or_default();
        let (certificate_chain, private_key) = certificate_provider
            .generate_server_certificate(self.name, self.uuid)?;
        let client_root_certificates = certificate_provider
            .client_root_certificates();
        let client_verifier = Arc::new(AllowAnyAuthenticatedClient::new((*client_root_certificates).clone()));
        let mut config = rustls::ServerConfig::builder()
            .with_safe_default_cipher_suites()
            .with_safe_default_kx_groups()
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .with_client_cert_verifier(client_verifier)
            .with_single_cert(certificate_chain, private_key)?;
        config.max_early_data_size = u32::MAX;
        let mut transport_config = TransportConfig::default();
        transport_config.max_idle_timeout(None);
        let mut server_config = ServerConfig::with_crypto(Arc::new(config));
        server_config.transport_config(Arc::new(transport_config));
        Ok(server_config)

    }
}

