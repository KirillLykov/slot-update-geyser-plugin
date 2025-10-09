use {
    agave_geyser_plugin_interface::geyser_plugin_interface::GeyserPluginError,
    serde::{Deserialize, Deserializer},
    std::net::SocketAddr,
    std::net::ToSocketAddrs,
    std::{fs::read_to_string, path::Path},
};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub libpath: String,
    #[serde(default)]
    pub log: LogConfig,
    #[serde(default)]
    pub tokio: TokioConfig,
    pub broadcaster: BroadcasterConfig,
}

impl Config {
    fn load_from_str(config: &str) -> std::result::Result<Self, GeyserPluginError> {
        serde_json::from_str(config).map_err(|error| GeyserPluginError::ConfigFileReadError {
            msg: error.to_string(),
        })
    }

    pub fn load_from_file<P: AsRef<Path>>(file: P) -> std::result::Result<Self, GeyserPluginError> {
        let config = read_to_string(file).map_err(GeyserPluginError::ConfigFileOpenError)?;
        Self::load_from_str(&config)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogConfig {
    /// Log level.
    #[serde(default = "LogConfig::default_level")]
    pub level: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: Self::default_level(),
        }
    }
}

impl LogConfig {
    fn default_level() -> String {
        "info".to_owned()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokioConfig {
    /// Number of worker threads in Tokio runtime
    pub worker_threads: usize,

    pub thread_name: String,
}

impl Default for TokioConfig {
    fn default() -> Self {
        Self {
            worker_threads: Self::default_worker_threads(),
            thread_name: Self::default_thread_name(),
        }
    }
}

impl TokioConfig {
    fn default_worker_threads() -> usize {
        4
    }

    fn default_thread_name() -> String {
        "tokio-worker".to_string()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BroadcasterConfig {
    /// Address of Grpc service.
    #[serde(deserialize_with = "deserialize_resolvable_socket_addr")]
    pub bind_address: SocketAddr,
    /// Address of the destination to send messages.
    #[serde(deserialize_with = "deserialize_resolvable_socket_addr")]
    pub target_address: SocketAddr,
    /// Capacity of the channel used to communicate with broadcaster task.
    pub channel_capacity: usize,
}

fn deserialize_resolvable_socket_addr<'de, D>(deserializer: D) -> Result<SocketAddr, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if let Ok(addr) = s.parse::<SocketAddr>() {
        return Ok(addr);
    }
    // Try system resolver (/etc/hosts + DNS)
    s.to_socket_addrs()
        .map_err(serde::de::Error::custom)?
        .next()
        .ok_or_else(|| serde::de::Error::custom(format!("Failed to resolve address: {s}")))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::fs::File,
        std::io::Write,
        std::net::{Ipv4Addr, SocketAddr},
        tempfile::tempdir,
    };

    fn sample_json() -> String {
        format!(
            r#"{{
                "libpath": "/tmp/libtest.so",
                "log": {{ "level": "debug" }},
                "tokio": {{ "worker_threads": 8, "thread_name": "custom" }},
                "broadcaster": {{
                    "bind_address": "127.0.0.1:8000",
                    "target_address": "127.0.0.1:9000",
                    "channel_capacity": 10
                }}
            }}"#
        )
    }

    #[test]
    fn test_load_from_str_valid() {
        let cfg = Config::load_from_str(&sample_json()).unwrap();
        assert_eq!(cfg.libpath, "/tmp/libtest.so");
        assert_eq!(cfg.log.level, "debug");
        assert_eq!(cfg.tokio.worker_threads, 8);
        assert_eq!(cfg.tokio.thread_name, "custom");
        assert_eq!(
            cfg.broadcaster.bind_address,
            SocketAddr::from((Ipv4Addr::LOCALHOST, 8000))
        );
        assert_eq!(
            cfg.broadcaster.target_address,
            SocketAddr::from((Ipv4Addr::LOCALHOST, 9000))
        );
        assert_eq!(cfg.broadcaster.channel_capacity, 10);
    }

    #[test]
    fn test_load_from_str_with_defaults() {
        let json = r#"
        {
            "libpath": "/libtest.so",
            "broadcaster": {
                "bind_address": "127.0.0.1:1000",
                "target_address": "127.0.0.1:2000",
                "channel_capacity": 1
            }
        }"#;

        let cfg = Config::load_from_str(json).unwrap();
        assert_eq!(cfg.log.level, "info");
        assert_eq!(cfg.tokio.worker_threads, 4);
        assert_eq!(cfg.tokio.thread_name, "tokio-worker");
    }

    #[test]
    fn test_invalid_json_returns_error() {
        let json = r#"{ "libpath": "x", "broadcaster": {} }"#;
        let err = Config::load_from_str(json).unwrap_err();
        match err {
            GeyserPluginError::ConfigFileReadError { msg } => {
                assert!(msg.contains("missing field"));
            }
            _ => panic!("Unexpected error type"),
        }
    }

    #[test]
    fn test_load_from_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut file = File::create(&path).unwrap();
        file.write_all(sample_json().as_bytes()).unwrap();

        let cfg = Config::load_from_file(&path).unwrap();
        assert_eq!(cfg.log.level, "debug");
    }

    #[test]
    fn test_deserialize_resolvable_socket_addr_parses_ip() {
        let json = r#""127.0.0.1:8080""#;
        let addr: SocketAddr =
            deserialize_resolvable_socket_addr(&mut serde_json::Deserializer::from_str(json))
                .expect("should parse IP:port");
        assert_eq!(addr, SocketAddr::from(([127, 0, 0, 1], 8080)));
    }
}
