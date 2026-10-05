//! TEST-ONLY Chrome-shaped TLS/HTTP2 handshake profile (Z2-T1).
//!
//! Compiled only with `--features impersonate-test`, which no shipped crate
//! enables (pinned by `tests/impersonate_pin.rs`). It changes the handshake
//! SHAPE only: ClientHello (GREASE, extension order permutation, cipher /
//! curve / sigalg lists, ALPN, ALPS, brotli cert compression) and the h2
//! SETTINGS. It sends the engine's honest User-Agent, answers no challenge,
//! and executes no script.

use std::io::Write;

use boring::ssl::{
    CertificateCompressionAlgorithm, CertificateCompressor, SslConnector, SslMethod, SslVerifyMode,
    SslVersion,
};
use foreign_types::ForeignTypeRef;
use tokio::net::TcpStream;
use tokio_boring::SslStream;

use crate::{HttpError, NegotiatedProtocol};

const CHROME_CIPHERS: &str = "ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:\
ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384:\
ECDHE-ECDSA-CHACHA20-POLY1305:ECDHE-RSA-CHACHA20-POLY1305:\
ECDHE-RSA-AES128-SHA:ECDHE-RSA-AES256-SHA:AES128-GCM-SHA256:AES256-GCM-SHA384:\
AES128-SHA:AES256-SHA";
const CHROME_SIGALGS: &str = "ecdsa_secp256r1_sha256:rsa_pss_rsae_sha256:rsa_pkcs1_sha256:\
ecdsa_secp384r1_sha384:rsa_pss_rsae_sha384:rsa_pkcs1_sha384:rsa_pss_rsae_sha512:rsa_pkcs1_sha512";
const CHROME_CURVES: &str = "X25519MLKEM768:X25519:P-256:P-384";
const ALPN_WIRE: &[u8] = b"\x02h2\x08http/1.1";

struct BrotliDecompress;

impl CertificateCompressor for BrotliDecompress {
    const ALGORITHM: CertificateCompressionAlgorithm = CertificateCompressionAlgorithm::BROTLI;
    const CAN_COMPRESS: bool = false;
    const CAN_DECOMPRESS: bool = true;

    fn decompress<W: Write>(&self, input: &[u8], output: &mut W) -> std::io::Result<()> {
        let mut reader = brotli_decompressor::Decompressor::new(input, 4096);
        std::io::copy(&mut reader, output).map(|_| ())
    }
}

/// A Chrome-shaped BoringSSL client connector.
#[derive(Clone)]
pub(crate) struct ChromeProfile {
    connector: SslConnector,
}

impl ChromeProfile {
    pub(crate) fn new() -> Result<Self, HttpError> {
        let tls = |e: boring::error::ErrorStack| HttpError::TlsError(format!("impersonate: {e}"));
        let mut b = SslConnector::builder(SslMethod::tls()).map_err(tls)?;
        b.set_default_verify_paths().map_err(tls)?;
        b.set_verify(SslVerifyMode::PEER);
        b.set_min_proto_version(Some(SslVersion::TLS1_2)).map_err(tls)?;
        b.set_max_proto_version(Some(SslVersion::TLS1_3)).map_err(tls)?;
        b.set_grease_enabled(true);
        b.set_permute_extensions(true);
        b.enable_ocsp_stapling();
        b.enable_signed_cert_timestamps();
        b.set_cipher_list(CHROME_CIPHERS).map_err(tls)?;
        b.set_sigalgs_list(CHROME_SIGALGS).map_err(tls)?;
        b.set_curves_list(CHROME_CURVES).map_err(tls)?;
        b.set_alpn_protos(ALPN_WIRE).map_err(tls)?;
        b.add_certificate_compression_algorithm(BrotliDecompress).map_err(tls)?;
        Ok(Self { connector: b.build() })
    }

    pub(crate) async fn connect(
        &self,
        host: &str,
        stream: TcpStream,
    ) -> Result<(SslStream<TcpStream>, NegotiatedProtocol), HttpError> {
        let tls = |e: boring::error::ErrorStack| HttpError::TlsError(format!("impersonate: {e}"));
        let config = self.connector.configure().map_err(tls)?;
        config.set_enable_ech_grease(true);
        let ssl_ptr = {
            let ssl_ref: &boring::ssl::SslRef = &config;
            ssl_ref.as_ptr()
        };
        // ALPS (application_settings, h2, empty client settings) is per-connection.
        unsafe {
            boring_sys::SSL_set_alps_use_new_codepoint(ssl_ptr, 1);
            boring_sys::SSL_add_application_settings(
                ssl_ptr,
                b"h2".as_ptr(),
                2,
                b"".as_ptr(),
                0,
            );
        }
        let ssl = tokio_boring::connect(config, host, stream)
            .await
            .map_err(|e| HttpError::TlsError(format!("impersonate handshake: {e}")))?;
        let negotiated = match ssl.ssl().selected_alpn_protocol() {
            Some(b"h2") => NegotiatedProtocol::H2,
            _ => NegotiatedProtocol::Http1,
        };
        Ok((ssl, negotiated))
    }
}

/// Chrome's h2 SETTINGS and connection window (observed, Chrome 13x).
pub(crate) fn chrome_h2_builder() -> h2::client::Builder {
    let mut b = h2::client::Builder::new();
    b.header_table_size(65_536)
        .enable_push(false)
        .initial_window_size(6_291_456)
        .initial_connection_window_size(15_728_640)
        .max_header_list_size(262_144);
    b
}
