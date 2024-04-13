use std::io::Cursor;
use std::num::NonZeroUsize;

use rcgen::{CertificateParams, CustomExtension, DnType, KeyPair, SanType};
use rustls::{Certificate, PrivateKey, RootCertStore};
use shared::certificate::*;
use std::sync::Arc;
use std::thread;
use shared::platform::Platform;
use uuid::Uuid;
#[derive(Debug)]
pub struct CertificateProvider {
    server_root_certificates: Arc<RootCertStore>,
}

impl CertificateProvider {
    pub fn server_root_certificates(&self) -> Arc<RootCertStore> {
        self.server_root_certificates.clone()
    }
}

impl Default for CertificateProvider {
    fn default() -> Self {
        let mut server_root_certificates = RootCertStore::empty();

        let server_root_certificate = rustls_pemfile::certs(&mut Cursor::new(include_bytes!(
            "certificates/client_root_certificate.pem"
        )))
        .unwrap()
        .into_iter()
        .next()
        .map(Certificate)
        .unwrap();
        server_root_certificates
            .add(&server_root_certificate)
            .unwrap();
        let server_root_certificates = Arc::new(server_root_certificates);

        Self {
            server_root_certificates,
        }
    }
}
