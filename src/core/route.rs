#[derive(Debug, Clone)]
pub(crate) struct RouteRule {
    /// 匹配条件
    pub(crate) match_: Match,
    /// 转发信息
    pub(crate) forward: Forward,
}

impl RouteRule {
    pub(crate) fn new(
        match_host: &str,
        match_path_prefix: &str,
        forward_host: &str,
        forward_path_prefix: &str,
    ) -> Self {
        Self {
            match_: Match {
                host: match_host.to_string(),
                prefix: match_path_prefix.to_string(),
            },
            forward: Forward {
                host: forward_host.to_string(),
                prefix: forward_path_prefix.to_string(),
                rewrite: true,
                connect_fail_use_original_host: false,
            },
        }
    }
    fn matches(&self, host: &str, prefix: &str) -> bool {
        (prefix.starts_with(&self.match_.prefix))
            && (host == self.match_.host || self.match_.host == "*")
    }
    fn match_host(&self, host: &str) -> bool {
        host == self.match_.host || self.match_.host == "*"
    }
}

/// 匹配
#[derive(Clone, Debug)]
pub(crate) struct Match {
    /// 匹配目标域名或IP:PORT，也可以是 * 匹配所有
    pub(crate) host: String,
    /// 匹配目标请求地址前缀
    pub(crate) prefix: String,
}

/// 转发
#[derive(Clone, Debug)]
pub struct Forward {
    /// 转发到目标地址
    pub(crate) host: String,
    /// 转发到目标地址的前缀
    pub(crate) prefix: String,
    /// 自动替换前缀 match.prefix替换为forward.prefix
    pub(crate) rewrite: bool,
    /// 转发地址连接失败时使用原始地址
    pub(crate) connect_fail_use_original_host: bool,
}

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

pub(crate) struct RouteEngine {
    pub(crate) rules: Arc<RwLock<Vec<RouteRule>>>,
}

impl RouteEngine {
    #[allow(unused)]
    pub(crate) async fn resolve_target(&self, host: &str, path: &str) -> Option<RouteRule> {
        debug!("Resolving target {host}{path}");
        let rules = self.rules.read().await;
        for rule in rules.iter() {
            // 匹配IP:PORT + 路径前缀
            if rule.matches(host, path) {
                return Some(rule.clone());
            }
        }
        None
    }

    pub(crate) async fn resolve_target_by_host(&self, host: &str) -> Option<RouteRule> {
        let rules = self.rules.read().await;
        for rule in rules.iter() {
            if rule.match_host(host) {
                return Some(rule.clone());
            }
        }
        None
    }

    // 动态更新规则
    #[allow(unused)]
    async fn update_rules(&self, new_rules: Vec<RouteRule>) {
        let mut rules = self.rules.write().await;
        *rules = new_rules;
    }
}
