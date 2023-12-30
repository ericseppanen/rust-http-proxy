use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;

use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};

/// Load a bunch of certificates from a PEM-formatted file.
// FIXME: support other formats (multiple files; DER)
pub fn load_certs(path: &Path) -> io::Result<Vec<CertificateDer<'static>>> {
    let file = File::open(path)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "failed to open certificate file"))?;

    let result = rustls_pemfile::certs(&mut BufReader::new(file))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "invalid X509 certificate"))?;
    Ok(result)
}

/// Load exactly one RSA private key from a file.
// FIXME: support other key types/formats.
pub fn load_private_key(path: &Path) -> io::Result<PrivateKeyDer> {
    let keyfile = File::open(path)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "failed to open private key file"))?;
    let mut reader = BufReader::new(keyfile);
    let key = rustls_pemfile::private_key(&mut reader)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "bad private key file"))?;
    let Some(key) = key else {
        return Err(io::Error::new(io::ErrorKind::Other, "no private key found"));
    };
    // return the first key. We know this will always succeed.
    Ok(key)
}
