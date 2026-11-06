//! Local certificate authority and short-lived leaf certificates.

use rcgen::{
    BasicConstraints, Certificate, CertificateParams, DistinguishedName, DnType,
    ExtendedKeyUsagePurpose, IsCa, KeyPair, KeyUsagePurpose,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use sha1::{Digest, Sha1};
use std::sync::Arc;
use thiserror::Error;
use time::{Duration, OffsetDateTime};

#[derive(Debug, Error)]
pub enum CaError {
    #[error("rcgen: {0}")]
    Rcgen(#[from] rcgen::Error),

    #[error("rustls: {0}")]
    Rustls(#[from] rustls::Error),

    #[error("informe pelo menos um host para o certificado leaf")]
    EmptyHosts,

    #[error("PEM inválido: {0}")]
    InvalidPem(String),
}

#[derive(Debug, Clone)]
pub struct CaMaterial {
    pub cert_pem: String,
    pub key_pem: String,
}

#[derive(Debug, Clone)]
pub struct LeafCertificate {
    cert_pem: String,
    key_pem: String,
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
}

impl LeafCertificate {
    pub fn cert_pem(&self) -> &str {
        &self.cert_pem
    }

    pub fn key_pem(&self) -> &str {
        &self.key_pem
    }

    pub fn server_config(&self) -> Result<rustls::ServerConfig, CaError> {
        let certs = vec![CertificateDer::from(self.cert_der.clone())];
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key_der.clone()));
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut config = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()?
            .with_no_client_auth()
            .with_single_cert(certs, key)?;
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        Ok(config)
    }
}

pub struct CertificateAuthority {
    key: KeyPair,
    cert: Certificate,
    cert_pem: String,
    key_pem: String,
    cert_der: Vec<u8>,
}

impl CertificateAuthority {
    pub fn generate() -> Result<Self, CaError> {
        let key = KeyPair::generate()?;
        let mut params = CertificateParams::new(Vec::<String>::new())?;
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature,
        ];
        let mut name = DistinguishedName::new();
        name.push(DnType::CommonName, "frank-figma-gateway local CA");
        name.push(DnType::OrganizationName, "frank-figma-gateway");
        params.distinguished_name = name;
        let now = OffsetDateTime::now_utc();
        params.not_before = now - Duration::days(1);
        params.not_after = now + Duration::days(365 * 2);

        let cert = params.clone().self_signed(&key)?;
        let cert_pem = cert.pem();
        let cert_der = cert.der().to_vec();
        let key_pem = key.serialize_pem();
        Ok(Self {
            key,
            cert,
            cert_pem,
            key_pem,
            cert_der,
        })
    }

    pub fn load(cert_pem: &str, key_pem: &str) -> Result<Self, CaError> {
        let params = CertificateParams::from_ca_cert_pem(cert_pem)
            .map_err(|error| CaError::InvalidPem(error.to_string()))?;
        let key =
            KeyPair::from_pem(key_pem).map_err(|error| CaError::InvalidPem(error.to_string()))?;
        let parsed =
            pem::parse(cert_pem).map_err(|error| CaError::InvalidPem(error.to_string()))?;
        let cert = params.clone().self_signed(&key)?;
        Ok(Self {
            key,
            cert_pem: cert_pem.to_owned(),
            key_pem: key_pem.to_owned(),
            cert_der: parsed.contents().to_vec(),
            cert,
        })
    }

    pub fn material(&self) -> CaMaterial {
        CaMaterial {
            cert_pem: self.cert_pem.clone(),
            key_pem: self.key_pem.clone(),
        }
    }

    pub fn cert_pem(&self) -> &str {
        &self.cert_pem
    }

    pub fn key_pem(&self) -> &str {
        &self.key_pem
    }

    pub fn cert_der(&self) -> &[u8] {
        &self.cert_der
    }

    pub fn thumbprint(&self) -> String {
        thumbprint(&self.cert_der)
    }

    pub fn leaf_for(&self, hosts: &[String]) -> Result<LeafCertificate, CaError> {
        if hosts.is_empty() {
            return Err(CaError::EmptyHosts);
        }
        let key = KeyPair::generate()?;
        let mut params = CertificateParams::new(hosts.to_vec())?;
        params.is_ca = IsCa::NoCa;
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let mut name = DistinguishedName::new();
        name.push(DnType::CommonName, hosts[0].as_str());
        params.distinguished_name = name;
        let now = OffsetDateTime::now_utc();
        params.not_before = now - Duration::days(1);
        params.not_after = now + Duration::days(90);

        let cert = params.signed_by(&key, &self.cert, &self.key)?;
        Ok(LeafCertificate {
            cert_pem: cert.pem(),
            cert_der: cert.der().to_vec(),
            key_der: key.serialize_der(),
            key_pem: key.serialize_pem(),
        })
    }
}

pub fn thumbprint_from_pem(cert_pem: &str) -> Result<String, CaError> {
    let parsed = pem::parse(cert_pem).map_err(|error| CaError::InvalidPem(error.to_string()))?;
    Ok(thumbprint(parsed.contents()))
}

pub fn thumbprint(der: &[u8]) -> String {
    let digest = Sha1::digest(der);
    digest
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use x509_parser::parse_x509_certificate;

    #[test]
    fn generates_a_ca_and_a_leaf() {
        let ca = CertificateAuthority::generate().expect("ca");
        assert!(ca.cert_pem().contains("BEGIN CERTIFICATE"));
        assert!(ca.key_pem().contains("BEGIN PRIVATE KEY"));
        assert_eq!(ca.thumbprint().len(), 40);
        assert!(ca.thumbprint().chars().all(|ch| ch.is_ascii_hexdigit()));

        let leaf = ca.leaf_for(&["api.figma.com".to_owned()]).expect("leaf");
        assert!(leaf.cert_pem().contains("BEGIN CERTIFICATE"));
        let (_, cert) = parse_x509_certificate(&leaf.cert_der).expect("parse leaf");
        assert!(cert.subject().to_string().contains("api.figma.com"));
        assert!(cert.issuer().to_string().contains("frank-figma-gateway"));
        leaf.server_config().expect("server config");
    }

    #[test]
    fn loads_a_persisted_ca_and_issues_leaves() {
        let ca = CertificateAuthority::generate().expect("ca");
        let material = ca.material();
        let loaded =
            CertificateAuthority::load(&material.cert_pem, &material.key_pem).expect("load");
        assert_eq!(loaded.thumbprint(), ca.thumbprint());
        assert!(loaded.leaf_for(&["api.figma.com".to_owned()]).is_ok());
    }

    #[test]
    fn rejects_empty_leaf_hosts() {
        let ca = CertificateAuthority::generate().expect("ca");
        assert!(matches!(ca.leaf_for(&[]), Err(CaError::EmptyHosts)));
    }
}
