use serde::Deserialize;
use std::io;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Once;

const CONFIG_FILE: &str = "./http_proxy_config.toml";

static mut GLOBAL_CONFIG: Option<Config> = None;
static INIT_CONFIG: Once = Once::new();

/// Initialize the global config state, and return a static ref to that config.
fn init_global_config(config: Config) -> &'static Config {
    // SAFETY: "static mut" variables require the unsafe keyword.
    // This is the standard way of initializing global singletons.
    // std::sync::Once ensures that only one thread will enter call_once,
    // and all other threads will block until it completes.
    unsafe {
        if GLOBAL_CONFIG.is_some() {
            panic!("global_config may only be initialized once");
        }
        INIT_CONFIG.call_once(|| {
            GLOBAL_CONFIG = Some(config);
        });
        GLOBAL_CONFIG.as_ref().unwrap()
    }
}

#[derive(Deserialize)]
pub struct Config {
    /// The IP address of the local interface to listen on (e.g. 127.0.0.1)
    pub local_addr: IpAddr,
    /// The local TCP port to bind to
    pub local_port: u16,
    /// The list of servers (host+port) that we will allow a connection to.
    pub allowed_servers: Vec<String>,
    /// Should the client<->proxy connection use TLS?
    pub use_tls: bool,
    /// A file containing our TLS certificate chain, in PEM format.
    pub cert_chain: Option<PathBuf>,
    /// A file containing our private key, in PEM format.
    pub private_key: Option<PathBuf>,
}

impl Config {
    pub fn read_config_file() -> io::Result<&'static Config> {
        let config_data = std::fs::read_to_string(CONFIG_FILE).map_err(|e| {
            eprintln!("failed to load toml config file");
            e
        })?;

        let config = toml::from_str(&config_data).map_err(|_| {
            io::Error::new(io::ErrorKind::Other, "failed to parse toml config file")
        })?;
        Ok(init_global_config(config))
    }

    pub fn is_server_allowed(&self, requested: &str) -> bool {
        for allowed_server in &self.allowed_servers {
            if requested == allowed_server {
                return true;
            }
        }
        false
    }

    /// Retrieve the cert and key filenames.
    ///
    /// Returns an error if either are None.
    pub fn get_cert_filenames(&self) -> io::Result<(&Path, &Path)> {
        match (&self.cert_chain, &self.private_key) {
            (Some(cert), Some(key)) => Ok((cert, key)),
            _ => Err(io::Error::new(
                io::ErrorKind::Other,
                "missing cert or key filename",
            )),
        }
    }
}
