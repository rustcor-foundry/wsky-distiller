use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
pub struct ProxyRotator {
    proxies: Vec<String>,
    index: AtomicUsize,
}

impl ProxyRotator {
    pub fn new(proxies: Vec<String>) -> Self {
        Self {
            proxies,
            index: AtomicUsize::new(0),
        }
    }

    pub fn next(&self) -> Option<String> {
        if self.proxies.is_empty() {
            return None;
        }

        let i = self.index.fetch_add(1, Ordering::Relaxed);
        Some(self.proxies[i % self.proxies.len()].clone())
    }
}
