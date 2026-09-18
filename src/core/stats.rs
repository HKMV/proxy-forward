use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

#[derive(Default)]
pub struct Stats {
    pub active: AtomicI64,
    pub total: AtomicU64,
    pub bytes_up: AtomicU64,
    pub bytes_down: AtomicU64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StatsSnapshot {
    pub active: i64,
    pub total: u64,
    pub bytes_up: u64,
    pub bytes_down: u64,
}

impl Stats {
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// 连接打开时调用，返回的 guard drop 时自动减活跃数
    pub fn conn_open(self: &Arc<Self>) -> ConnGuard {
        self.active.fetch_add(1, Ordering::Relaxed);
        self.total.fetch_add(1, Ordering::Relaxed);
        ConnGuard {
            stats: self.clone(),
        }
    }

    pub fn up(&self, n: usize) {
        self.bytes_up.fetch_add(n as u64, Ordering::Relaxed);
    }

    pub fn down(&self, n: usize) {
        self.bytes_down.fetch_add(n as u64, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            active: self.active.load(Ordering::Relaxed),
            total: self.total.load(Ordering::Relaxed),
            bytes_up: self.bytes_up.load(Ordering::Relaxed),
            bytes_down: self.bytes_down.load(Ordering::Relaxed),
        }
    }
}

pub struct ConnGuard {
    stats: Arc<Stats>,
}

impl Drop for ConnGuard {
    fn drop(&mut self) {
        self.stats.active.fetch_sub(1, Ordering::Relaxed);
    }
}
