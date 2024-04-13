use crate::endpoint_config::certificate_provider::CertificateProvider;
use std::sync::Arc;
use std::time::Duration;
use quinn::{ClientConfig, TransportConfig, VarInt};
use shared::certificate::ConfigError;
use shared::certificate::verifier::CertificateVerifier;

pub struct EndpointConfigBuilder {
    pub(crate) certificate_provider: Option<CertificateProvider>,
}
impl EndpointConfigBuilder {
    pub fn new() -> Self {
        Self {
            certificate_provider: None,
        }
    }

    pub fn certificate_provider(mut self, provider: CertificateProvider) -> Self {
        self.certificate_provider = Some(provider);
        self
    }
    pub fn build(self) -> Result<quinn::ClientConfig, ConfigError> {
        let certificate_provider = self
            .certificate_provider
            .unwrap_or_default();
        let server_root_certificates = certificate_provider
            .server_root_certificates();
        let mut client_config = rustls::ClientConfig::builder()
            .with_safe_default_cipher_suites()
            .with_safe_default_kx_groups()
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .with_custom_certificate_verifier(Arc::new(CertificateVerifier::new(server_root_certificates)))
            .with_no_client_auth();
        client_config.enable_early_data = true;
        client_config.enable_sni = false;

        let mut transport_config = TransportConfig::default();
        transport_config.max_idle_timeout(Some(VarInt::from_u32(12_000).into()));
        transport_config.keep_alive_interval(Some(Duration::from_millis(600)));
        let transport_config = Arc::new(transport_config);
        let mut client_config = ClientConfig::new(Arc::new(client_config));
        client_config.transport_config(transport_config);
        Ok(client_config)
    }
}


