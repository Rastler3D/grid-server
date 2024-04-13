use std::io::Cursor;

use rcgen::{
    CertificateParams, CustomExtension, DistinguishedName, DnType, KeyPair, RcgenError, SanType,
};
use rustls::{Certificate, PrivateKey, RootCertStore};
use shared::certificate::{ServiceType, SERVICE_TYPE_OID};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug)]
pub struct CertificateProvider {
    client_root_certificates: Arc<RootCertStore>,
    server_root_certificate: Certificate,
    server_root_private_key: PrivateKey,
}

impl CertificateProvider {
    pub fn client_root_certificates(&self) -> Arc<RootCertStore> {
        self.client_root_certificates.clone()
    }
    pub fn generate_server_certificate(
        &self,
        name: Option<String>,
        uuid: Uuid
    ) -> Result<(Vec<Certificate>, PrivateKey), rcgen::Error> {
        let mut certificate_params = CertificateParams::default();
        certificate_params
            .distinguished_name
            .push(DnType::CommonName, uuid.to_string());
        if let Some(name) = name{
            certificate_params
                .subject_alt_names
                .push(SanType::DnsName(name));
        }
        certificate_params
            .custom_extensions
            .push(CustomExtension::from_oid_content(
                SERVICE_TYPE_OID,
                bincode::serde::encode_to_vec(
                    ServiceType::NodeManager,
                    bincode::config::standard(),
                )
                .unwrap(),
            ));

        let key_pair = KeyPair::from_der(&self.server_root_private_key.0)?;
        let root_certificate =
            CertificateParams::from_ca_cert_der(&self.server_root_certificate.0, key_pair)?;
        let root_certificate = rcgen::Certificate::from_params(root_certificate)?;
        let certificate = rcgen::Certificate::from_params(certificate_params)?;
        let private_key = PrivateKey(certificate.serialize_private_key_der());
        let certificate = Certificate(certificate.serialize_der_with_signer(&root_certificate)?);

        Ok((
            vec![certificate, self.server_root_certificate.clone()],
            private_key,
        ))
    }
}

impl Default for CertificateProvider {
    fn default() -> Self {
        let mut client_root_certificates = RootCertStore::empty();

        let client_root_certificate = rustls_pemfile::certs(&mut Cursor::new(include_bytes!(
            "certificates/client_root_certificate.pem"
        )))
        .unwrap()
        .into_iter()
        .next()
        .map(Certificate)
        .unwrap();
        client_root_certificates
            .add(&client_root_certificate)
            .unwrap();
        let client_root_certificates = Arc::new(client_root_certificates);
        let server_root_certificate = rustls_pemfile::certs(&mut Cursor::new(include_bytes!(
            "certificates/server_root_certificate.pem"
        )))
        .unwrap()
        .into_iter()
        .next()
        .map(Certificate)
        .unwrap();
        let server_root_private_key = rustls_pemfile::pkcs8_private_keys(&mut Cursor::new(
            include_bytes!("certificates/server_private_key.pem"),
        ))
        .unwrap()
        .into_iter()
        .next()
        .map(PrivateKey)
        .unwrap();

        Self {
            server_root_certificate,
            server_root_private_key,
            client_root_certificates,
        }
    }
}
