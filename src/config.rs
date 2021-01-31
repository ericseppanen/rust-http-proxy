use serde::Deserialize;
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
    pub local_addr: String, // FIXME: make this an IPAddr
    /// The local TCP port to bind to
    pub local_port: u16,
    /// The target host to proxy to
    pub target_host: String,
    /// The target port to proxy to
    pub target_port: u16,
}

impl Config {
    pub fn read_config_file() -> &'static Config {
        // FIXME: read this from a file.
        let config = Config {
            local_addr: "127.0.0.1".to_string(),
            local_port: 8080,
            target_host: "localhost".to_string(),
            target_port: 443,
        };
        init_global_config(config)
    }
}
