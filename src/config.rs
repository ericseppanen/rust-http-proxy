use serde::Deserialize;
use std::net::IpAddr;
use std::sync::Once;

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
    pub local_addr: IpAddr, // FIXME: make this an IPAddr
    /// The local TCP port to bind to
    pub local_port: u16,
    /// The list of servers (host+port) that we will allow a connection to.
    pub allowed_servers: Vec<String>,
}

impl Config {
    pub fn read_config_file() -> &'static Config {
        // FIXME: read this from a file.
        let config = Config {
            local_addr: "127.0.0.1".parse().unwrap(),
            local_port: 8080,
            allowed_servers: vec!["www.google.com:443".into()],
        };
        init_global_config(config)
    }

    pub fn is_server_allowed(&self, requested: &str) -> bool {
        for allowed_server in &self.allowed_servers {
            if requested == allowed_server {
                return true;
            }
        }
        false
    }
}
