use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;
use tokio_rustls::rustls::internal::pemfile::{certs, pkcs8_private_keys};
use tokio_rustls::rustls::{Certificate, PrivateKey};

/// Load a bunch of certificates from a PEM-formatted file.
// FIXME: support other formats (multiple files; DER)
pub fn load_certs(path: &Path) -> io::Result<Vec<Certificate>> {
    let file = File::open(path)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "failed to open certificate file"))?;
    let result = certs(&mut BufReader::new(file))
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "invalid X509 certificate"))?;
    Ok(result)
}

/// Load exactly one RSA private key from a file.
// FIXME: support other key types/formats.
pub fn load_private_key(path: &Path) -> io::Result<PrivateKey> {
    let keyfile = File::open(path)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "failed to open private key file"))?;
    let mut reader = BufReader::new(keyfile);
    let keys = pkcs8_private_keys(&mut reader)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "bad private key file"))?;
    if keys.len() != 1 {
        return Err(io::Error::new(io::ErrorKind::Other, "expected only 1 key"));
    }
    // return the first key. We know this will always succeed.
    Ok(keys.into_iter().next().unwrap())
}
