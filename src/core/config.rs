use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AppConfig {
    /// 规则列表
    pub rules: Vec<Rule>,
    pub listen_addr: String,
}
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            rules: vec![Rule {
                //默认示例
                matcher: Host {
                    addr: "192.168.120.177:81".to_string(),
                    path_prefix: "/api".to_string(),
                },
                forward: Host {
                    addr: "127.0.0.1:8686".to_string(),
                    path_prefix: "".to_string(),
                },
            }],
            listen_addr: "127.0.0.1:1080".to_string(),
        }
    }
}
impl AppConfig {
    pub(crate) fn init() -> anyhow::Result<Self> {
        let conf_file_path = "config.toml";
        match std::fs::read_to_string(conf_file_path) {
            Ok(content) => Ok(toml::from_str(&content)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // 首次运行生成默认配置
                let config = AppConfig::default();
                std::fs::write(conf_file_path, toml::to_string(&config)?)?;
                Ok(config)
            }
            Err(e) => Err(e.into()),
        }
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
