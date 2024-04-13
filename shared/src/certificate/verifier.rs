use std::sync::Arc;
use std::time::SystemTime;
use rustls::{Certificate, Error, RootCertStore, ServerName};
use rustls::client::{CertificateTransparencyPolicy, ServerCertVerified, ServerCertVerifier, verify_server_cert_signed_by_trust_anchor};
use rustls::server::ParsedCertificate;

impl ServerCertVerifier for CertificateVerifier {

    fn verify_server_cert(
        &self,
        end_entity: &Certificate,
        intermediates: &[Certificate],
        server_name: &ServerName,
        scts: &mut dyn Iterator<Item = &[u8]>,
        ocsp_response: &[u8],
        now: SystemTime,
    ) -> Result<ServerCertVerified, Error> {
        let cert = ParsedCertificate::try_from(end_entity)?;

        verify_server_cert_signed_by_trust_anchor(&cert, &self.roots, intermediates, now)?;

        Ok(ServerCertVerified::assertion())
    }
}

pub struct CertificateVerifier {
    roots: Arc<RootCertStore>,
}

impl CertificateVerifier {
    pub fn new(
        roots: impl Into<Arc<RootCertStore>>,
    ) -> Self {
        Self {
            roots: roots.into(),
        }
    }
}