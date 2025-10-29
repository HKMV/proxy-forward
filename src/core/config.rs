use config::{Config, ConfigError, File};
use serde::{Deserialize, Serialize};
use std::io::Write;
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AppConfig {
    /// 规则列表
    pub rules: Vec<Rule>,
    pub listen_addr: String,
}
impl Default for AppConfig {
    fn default() -> Self {
        let mut config = Self {
            rules: Vec::new(),
            listen_addr: "127.0.0.1:1080".to_string(),
        };
        //默认示例
        config.rules.push(Rule {
            matcher: Host {
                addr: "192.168.120.177:81".to_string(),
                path_prefix: "/api".to_string(),
            },
            forward: Host {
                addr: "127.0.0.1:8686".to_string(),
                path_prefix: "".to_string(),
            },
        });

        config
    }
}
impl AppConfig {
    pub(crate) fn init() -> Result<Self, ConfigError> {
        let conf_file_path = "config.toml";
        let result = std::fs::File::open(conf_file_path);
        if result.is_err() {
            let mut file = std::fs::File::create(conf_file_path).unwrap();
            let config = AppConfig::default();
            let ac = toml::to_string(&config).unwrap_or("".into());
            file.write(ac.as_ref()).unwrap();
            file.flush().unwrap();
        }

        let c = Config::builder()
            .add_source(File::with_name(conf_file_path))
            .build()?;

        c.try_deserialize()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rule {
    /// 匹配配置
    pub matcher: Host,
    /// 转发配置
    pub forward: Host,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Host {
    /// host:port
    pub addr: String,
    /// 路径前缀
    pub path_prefix: String,
}
