use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::Whois;

/// Whois results by remote IP, each kept for `ttl`.
pub(crate) struct WhoisCache {
    ttl: Duration,
    entries: HashMap<IpAddr, (Instant, Arc<Whois>)>,
}

impl WhoisCache {
    pub(crate) fn new(ttl: Duration) -> WhoisCache {
        WhoisCache {
            ttl,
            entries: HashMap::new(),
        }
    }

    pub(crate) fn get(&self, ip: IpAddr, now: Instant) -> Option<Arc<Whois>> {
        let (at, whois) = self.entries.get(&ip)?;
        (now.saturating_duration_since(*at) < self.ttl).then(|| whois.clone())
    }

    pub(crate) fn insert(&mut self, ip: IpAddr, whois: Arc<Whois>, now: Instant) {
        let ttl = self.ttl;
        self.entries
            .retain(|_, (at, _)| now.saturating_duration_since(*at) < ttl);
        self.entries.insert(ip, (now, whois));
    }

    pub(crate) fn remove(&mut self, ip: IpAddr) {
        self.entries.remove(&ip);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TTL: Duration = Duration::from_secs(300);

    fn whois(name: &str) -> Arc<Whois> {
        Arc::new(Whois {
            node_name: name.into(),
            ..Whois::default()
        })
    }

    fn ip(last: u8) -> IpAddr {
        IpAddr::from([100, 64, 0, last])
    }

    #[test]
    fn hit_within_ttl_miss_after() {
        let t0 = Instant::now();
        let mut cache = WhoisCache::new(TTL);
        assert!(cache.get(ip(2), t0).is_none());
        cache.insert(ip(2), whois("laptop"), t0);
        let hit = cache.get(ip(2), t0 + Duration::from_secs(299)).unwrap();
        assert_eq!(hit.node_name, "laptop");
        assert!(cache.get(ip(2), t0 + TTL).is_none());
        assert!(cache.get(ip(3), t0).is_none());
    }

    #[test]
    fn insert_replaces_and_restarts_the_clock() {
        let t0 = Instant::now();
        let mut cache = WhoisCache::new(TTL);
        cache.insert(ip(2), whois("old"), t0);
        let t1 = t0 + Duration::from_secs(200);
        cache.insert(ip(2), whois("new"), t1);
        let hit = cache.get(ip(2), t1 + Duration::from_secs(200)).unwrap();
        assert_eq!(hit.node_name, "new");
    }

    #[test]
    fn remove_forgets_one_ip() {
        let t0 = Instant::now();
        let mut cache = WhoisCache::new(TTL);
        cache.insert(ip(2), whois("a"), t0);
        cache.insert(ip(3), whois("b"), t0);
        cache.remove(ip(2));
        assert!(cache.get(ip(2), t0).is_none());
        assert!(cache.get(ip(3), t0).is_some());
    }

    #[test]
    fn insert_drops_expired_entries() {
        let t0 = Instant::now();
        let mut cache = WhoisCache::new(TTL);
        cache.insert(ip(2), whois("a"), t0);
        cache.insert(ip(3), whois("b"), t0 + TTL);
        assert_eq!(cache.entries.len(), 1);
    }
}
