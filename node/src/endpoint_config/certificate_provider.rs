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
    client_root_certificate: Certificate,
    client_root_private_key: PrivateKey,
}

impl CertificateProvider {
    pub fn server_root_certificates(&self) -> Arc<RootCertStore> {
        self.server_root_certificates.clone()
    }
    pub fn generate_client_certificate(
        &self,
        client_name: Option<String>,
        platform: Platform,
    ) -> Result<(Vec<Certificate>, PrivateKey), rcgen::Error> {
        let mut certificate_params = CertificateParams::default();
        certificate_params
            .custom_extensions
            .push(CustomExtension::from_oid_content(
                PLATFORM_OID,
                bincode::serde::encode_to_vec(
                    platform,
                    bincode::config::standard(),
                ).unwrap(),
            ));
        certificate_params
            .custom_extensions
            .push(CustomExtension::from_oid_content(
                SUBWORKERS_OID,
                thread::available_parallelism()
                    .map(NonZeroUsize::get)
                    .unwrap_or(1)
                    .to_be_bytes()
                    .to_vec(),
            ));
        certificate_params
            .custom_extensions
            .push(CustomExtension::from_oid_content(
                SERVICE_TYPE_OID,
                bincode::serde::encode_to_vec(
                    ServiceType::Node,
                    bincode::config::standard(),
                )
                .unwrap(),
            ));
        certificate_params
            .distinguished_name
            .push(DnType::CommonName, Uuid::new_v4());
        if let Some(client_name) = client_name {
            certificate_params
                .subject_alt_names
                .push(SanType::DnsName(client_name));
        }

        let key_pair = KeyPair::from_der(&self.client_root_private_key.0)?;
        let root_certificate =
            CertificateParams::from_ca_cert_der(&self.client_root_certificate.0, key_pair)?;
        let root_certificate = rcgen::Certificate::from_params(root_certificate)?;
        let certificate = rcgen::Certificate::from_params(certificate_params)?;
        let private_key = PrivateKey(certificate.serialize_private_key_der());
        let certificate = Certificate(certificate.serialize_der_with_signer(&root_certificate)?);

        Ok((
            vec![certificate, self.client_root_certificate.clone()],
            private_key,
        ))
    }
}

impl Default for CertificateProvider {
    fn default() -> Self {
        let mut server_root_certificates = RootCertStore::empty();

        let server_root_certificate = rustls_pemfile::certs(&mut Cursor::new(include_bytes!(
            "certificates/server_root_certificate.pem"
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
        let client_root_certificate = rustls_pemfile::certs(&mut Cursor::new(include_bytes!(
            "certificates/client_root_certificate.pem"
        )))
        .unwrap()
        .into_iter()
        .next()
        .map(Certificate)
        .unwrap();
        let client_root_private_key = rustls_pemfile::pkcs8_private_keys(&mut Cursor::new(
            include_bytes!("certificates/client_private_key.pem"),
        ))
        .unwrap()
        .into_iter()
        .next()
        .map(PrivateKey)
        .unwrap();

        Self {
            client_root_certificate,
            client_root_private_key,
            server_root_certificates,
        }
    }
}
