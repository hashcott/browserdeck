use std::net::Ipv4Addr;

/// Only private LAN ranges (10/8, 172.16/12, 192.168/16), 192.168 first, then 10, then 172.
pub fn rank_ips(ips: Vec<Ipv4Addr>) -> Vec<Ipv4Addr> {
    let rank = |ip: &Ipv4Addr| match ip.octets() {
        [192, 168, ..] => Some(0),
        [10, ..] => Some(1),
        [172, b, ..] if (16..=31).contains(&b) => Some(2),
        _ => None,
    };
    let mut out: Vec<_> = ips.into_iter().filter_map(|ip| rank(&ip).map(|r| (r, ip))).collect();
    out.sort_by_key(|(r, _)| *r); // stable: interface order kept within a class
    out.dedup_by_key(|(_, ip)| *ip);
    out.into_iter().map(|(_, ip)| ip).collect()
}

/// `rank_ips` over every network interface.
pub fn lan_ips() -> Vec<Ipv4Addr> {
    let all = if_addrs::get_if_addrs().unwrap_or_default();
    rank_ips(
        all.into_iter()
            .filter_map(|i| match i.ip() {
                std::net::IpAddr::V4(v4) => Some(v4),
                _ => None,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> Ipv4Addr {
        s.parse().unwrap()
    }

    #[test]
    fn ranks_private_addresses_192_first() {
        let got = rank_ips(vec![ip("10.0.0.5"), ip("192.168.1.20"), ip("172.16.0.3")]);
        assert_eq!(got, vec![ip("192.168.1.20"), ip("10.0.0.5"), ip("172.16.0.3")]);
    }

    #[test]
    fn drops_loopback_link_local_and_public() {
        let got = rank_ips(vec![ip("127.0.0.1"), ip("169.254.1.1"), ip("8.8.8.8"), ip("172.32.0.1"), ip("172.15.0.1")]);
        assert!(got.is_empty());
    }

    #[test]
    fn keeps_172_31_and_drops_duplicates() {
        assert_eq!(rank_ips(vec![ip("172.31.255.1"), ip("172.31.255.1")]), vec![ip("172.31.255.1")]);
    }
}
