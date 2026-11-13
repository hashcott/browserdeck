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

/// Every address of every interface of this PC.
pub fn own_ips() -> Vec<std::net::IpAddr> {
    if_addrs::get_if_addrs().unwrap_or_default().into_iter().map(|i| i.ip()).collect()
}

/// What the elevated app accepts as the phone: a private LAN address (as `rank_ips` keeps) that is not this PC.
/// Refuses the internet too, which a program here can reach the server from through a UPnP port forward.
pub fn may_be_the_phone(peer: std::net::IpAddr, own: &[std::net::IpAddr]) -> bool {
    let lan = match peer.to_canonical() {
        std::net::IpAddr::V4(v4) => !rank_ips(vec![v4]).is_empty(),
        std::net::IpAddr::V6(_) => false,
    };
    lan && !from_this_pc(peer, own)
}

/// Whether a client at `peer` runs on this PC: loopback, or one of `own` (this PC's addresses).
pub fn from_this_pc(peer: std::net::IpAddr, own: &[std::net::IpAddr]) -> bool {
    let peer = peer.to_canonical();
    peer.is_loopback() || own.iter().any(|ip| ip.to_canonical() == peer)
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
    fn from_this_pc_is_loopback_or_an_own_address() {
        let own: [std::net::IpAddr; 2] = ["192.168.1.20".parse().unwrap(), "fe80::1".parse().unwrap()];
        for p in ["127.0.0.1", "::1", "192.168.1.20", "::ffff:192.168.1.20", "fe80::1"] {
            assert!(from_this_pc(p.parse().unwrap(), &own), "{p}");
        }
        for p in ["192.168.1.50", "10.0.0.2", "::ffff:192.168.1.50"] {
            assert!(!from_this_pc(p.parse().unwrap(), &own), "{p}");
        }
    }

    #[test]
    fn the_phone_is_a_private_lan_address_that_is_not_this_pc() {
        let own: [std::net::IpAddr; 1] = ["192.168.1.20".parse().unwrap()];
        for p in ["192.168.1.50", "10.0.0.2", "172.16.4.4", "::ffff:192.168.1.50"] {
            assert!(may_be_the_phone(p.parse().unwrap(), &own), "{p}");
        }
        // this PC, the internet (a UPnP-forwarded port), carrier NAT, link-local
        for p in ["192.168.1.20", "127.0.0.1", "8.8.8.8", "100.64.0.5", "169.254.3.3", "::1"] {
            assert!(!may_be_the_phone(p.parse().unwrap(), &own), "{p}");
        }
    }

    #[test]
    fn keeps_172_31_and_drops_duplicates() {
        assert_eq!(rank_ips(vec![ip("172.31.255.1"), ip("172.31.255.1")]), vec![ip("172.31.255.1")]);
    }
}
