//! Private prototype address policy. This file is not wired into the runtime yet.
//! Its special-purpose ranges are pinned to the adjacent IANA CSV snapshots.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Instant,
};

#[path = "iana_tables.rs"]
mod iana_tables;

// This unadopted socket boundary is exercised with owned loopback tests only.
// The trusted catalog/auth/ledger constructor is a separate integration gate.
#[path = "socket_pin.rs"]
pub(in crate::http_client) mod socket_pin;

// Private Q1 plan and test-only native adapter; production wiring is separate.
#[path = "q1_dial.rs"]
pub(in crate::http_client) mod q1_dial;

const MAX_CANDIDATES: usize = 1024;
const MAX_EXCEPTIONS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ServiceIdentity {
    service: u64,
    purpose: u64,
    generation: u64,
    trust: u64,
}
impl ServiceIdentity {
    pub(in crate::http_client) fn from_startup(
        service: u64,
        purpose: u64,
        generation: u64,
        trust: u64,
    ) -> Result<Self, AddressError> {
        if [service, purpose, generation, trust].contains(&0) {
            return Err(AddressError::InvalidPolicy);
        }
        Ok(Self {
            service,
            purpose,
            generation,
            trust,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AddressError {
    InvalidPolicy,
    IdentityMismatch,
    Empty,
    TooManyCandidates,
    Disallowed,
    Expired,
    Capacity,
}

#[derive(Debug)]
pub(crate) struct InternalPolicy {
    identity: ServiceIdentity,
    port: u16,
    max_candidates: usize,
    exact_exceptions: Vec<IpAddr>,
}

impl InternalPolicy {
    pub(crate) fn checked(
        identity: ServiceIdentity,
        port: u16,
        max_candidates: usize,
        exact_exceptions: Vec<IpAddr>,
    ) -> Result<Self, AddressError> {
        if port == 0
            || max_candidates == 0
            || max_candidates > MAX_CANDIDATES
            || exact_exceptions.len() > MAX_EXCEPTIONS
        {
            return Err(AddressError::InvalidPolicy);
        }
        for (index, address) in exact_exceptions.iter().enumerate() {
            if exact_exceptions[..index].contains(address) {
                return Err(AddressError::InvalidPolicy);
            }
        }
        Ok(Self {
            identity,
            port,
            max_candidates,
            exact_exceptions,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AddressClass {
    Ordinary,
    IanaSpecial,
    Multicast,
    Reserved,
    Mapped(Ipv4Addr),
    TranslationTunnel,
}

fn classify(address: IpAddr) -> AddressClass {
    match address {
        IpAddr::V4(address) => {
            let first = address.octets()[0];
            if (224..240).contains(&first) {
                AddressClass::Multicast
            } else if first >= 240 {
                AddressClass::Reserved
            } else if iana_tables::IANA_V4
                .iter()
                .any(|&(base, prefix)| prefix_v4(u32::from(address), base, prefix))
            {
                AddressClass::IanaSpecial
            } else {
                AddressClass::Ordinary
            }
        }
        IpAddr::V6(address) => {
            let bits = u128::from(address);
            if address.octets()[0] == 0xff {
                AddressClass::Multicast
            } else if let Some(mapped) = address.to_ipv4_mapped() {
                AddressClass::Mapped(mapped)
            } else if TRANSLATION_TUNNEL_V6
                .iter()
                .any(|&(base, prefix)| prefix_v6(bits, base, prefix))
            {
                AddressClass::TranslationTunnel
            } else if iana_tables::IANA_V6
                .iter()
                .any(|&(base, prefix)| prefix_v6(bits, base, prefix))
            {
                AddressClass::IanaSpecial
            } else {
                AddressClass::Ordinary
            }
        }
    }
}

// Each prefix below also appears in the pinned IPv6 Special-Purpose CSV.
// Their separate class prevents treating embedded IPv4 values as a route decision.
const TRANSLATION_TUNNEL_V6: &[(u128, u8)] = &[
    (0x0064ff9b000000000000000000000000, 96), // NAT64
    (0x0064ff9b000100000000000000000000, 48), // local-use NAT64
    (0x20010000000000000000000000000000, 32), // Teredo
    (0x20020000000000000000000000000000, 16), // 6to4
];

fn prefix_v4(address: u32, base: u32, prefix: u8) -> bool {
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    (address & mask) == base
}

fn prefix_v6(address: u128, base: u128, prefix: u8) -> bool {
    let mask = if prefix == 0 {
        0
    } else {
        u128::MAX << (128 - prefix)
    };
    (address & mask) == base
}

fn permitted(policy: &InternalPolicy, address: IpAddr) -> bool {
    match classify(address) {
        AddressClass::Ordinary => true,
        AddressClass::Mapped(embedded) => {
            // A mapped exception is explicit for both the presented IPv6 and the
            // embedded IPv4; no implicit cross-family exception is inferred.
            policy.exact_exceptions.contains(&address)
                && policy.exact_exceptions.contains(&IpAddr::V4(embedded))
        }
        AddressClass::IanaSpecial
        | AddressClass::Multicast
        | AddressClass::Reserved
        | AddressClass::TranslationTunnel => policy.exact_exceptions.contains(&address),
    }
}

#[derive(Debug)]
pub(crate) struct CheckedSnapshot {
    identity: ServiceIdentity,
    observed_count: usize,
    checked_candidates: Vec<IpAddr>,
    selected: SocketAddr,
    valid_until: Instant,
    operation_end: Instant,
}

#[derive(Debug)]
pub(crate) struct DialSelection {
    identity: ServiceIdentity,
    selected: SocketAddr,
    valid_until: Instant,
    operation_end: Instant,
}

impl CheckedSnapshot {
    pub(crate) fn begin_dial(self, now: Instant) -> Result<DialSelection, AddressError> {
        if now >= self.valid_until || now >= self.operation_end {
            return Err(AddressError::Expired);
        }
        Ok(DialSelection {
            identity: self.identity,
            selected: self.selected,
            valid_until: self.valid_until,
            operation_end: self.operation_end,
        })
    }
}

pub(crate) fn check_candidates(
    policy: &InternalPolicy,
    expected_identity: ServiceIdentity,
    addresses: &[IpAddr],
    valid_until: Instant,
    operation_end: Instant,
    now: Instant,
) -> Result<CheckedSnapshot, AddressError> {
    if policy.identity != expected_identity {
        return Err(AddressError::IdentityMismatch);
    }
    if addresses.is_empty() {
        return Err(AddressError::Empty);
    }
    // Count the complete returned set before deduplication or class filtering.
    if addresses.len() > policy.max_candidates {
        return Err(AddressError::TooManyCandidates);
    }
    if now >= valid_until || now >= operation_end {
        return Err(AddressError::Expired);
    }
    if addresses
        .iter()
        .copied()
        .any(|address| !permitted(policy, address))
    {
        return Err(AddressError::Disallowed);
    }
    let mut checked_candidates = Vec::new();
    checked_candidates
        .try_reserve_exact(addresses.len())
        .map_err(|_| AddressError::Capacity)?;
    checked_candidates.extend_from_slice(addresses);
    checked_candidates.sort_unstable_by_key(|address| match address {
        IpAddr::V4(v4) => (0_u8, u128::from(u32::from(*v4))),
        IpAddr::V6(v6) => (1_u8, u128::from(*v6)),
    });
    checked_candidates.dedup();
    let selected = SocketAddr::new(checked_candidates[0], policy.port);
    Ok(CheckedSnapshot {
        identity: expected_identity,
        observed_count: addresses.len(),
        checked_candidates,
        selected,
        valid_until,
        operation_end,
    })
}

pub(crate) fn check_literal(
    policy: &InternalPolicy,
    expected_identity: ServiceIdentity,
    address: IpAddr,
    valid_until: Instant,
    operation_end: Instant,
    now: Instant,
) -> Result<CheckedSnapshot, AddressError> {
    check_candidates(
        policy,
        expected_identity,
        &[address],
        valid_until,
        operation_end,
        now,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{str::FromStr, time::Duration};

    const ID: ServiceIdentity = ServiceIdentity {
        service: 1,
        purpose: 2,
        generation: 3,
        trust: 4,
    };

    fn ip(text: &str) -> IpAddr {
        IpAddr::from_str(text).unwrap()
    }
    fn policy(exceptions: &[&str], max: usize) -> InternalPolicy {
        InternalPolicy::checked(ID, 443, max, exceptions.iter().map(|s| ip(s)).collect()).unwrap()
    }
    fn check(p: &InternalPolicy, addresses: &[IpAddr]) -> Result<CheckedSnapshot, AddressError> {
        let now = Instant::now();
        check_candidates(
            p,
            ID,
            addresses,
            now + Duration::from_secs(30),
            now + Duration::from_secs(60),
            now,
        )
    }

    #[test]
    fn pinned_iana_tables_equal_every_csv_block() {
        fn blocks(csv: &str, family: u8) -> Vec<(u128, u8)> {
            let mut result = Vec::new();
            for line in csv.lines().skip(1) {
                // This pinned CSV contains continuation lines in quoted RFC cells.
                if line.starts_with(' ') {
                    continue;
                }
                let line = line.trim_end_matches('\r');
                let first = if let Some(rest) = line.strip_prefix('"') {
                    rest.split_once('"').unwrap().0
                } else {
                    line.split_once(',').unwrap().0
                };
                for item in first.split(',') {
                    let cidr = item.trim().split_once(' ').map_or(item.trim(), |(v, _)| v);
                    let (address, prefix) = cidr.split_once('/').unwrap();
                    let address = IpAddr::from_str(address).unwrap();
                    assert_eq!(address.is_ipv4(), family == 4);
                    let bits: u128 = match address {
                        IpAddr::V4(v4) => u32::from(v4).into(),
                        IpAddr::V6(v6) => u128::from(v6),
                    };
                    result.push((bits, prefix.parse().unwrap()));
                }
            }
            result
        }
        let v4 = blocks(include_str!("iana-ipv4-special.csv"), 4);
        let v6 = blocks(include_str!("iana-ipv6-special.csv"), 6);
        assert_eq!(v4.len(), 26);
        assert_eq!(v6.len(), 25);
        assert_eq!(
            v4,
            iana_tables::IANA_V4
                .iter()
                .map(|&(n, p)| (u128::from(n), p))
                .collect::<Vec<_>>()
        );
        assert_eq!(v6, iana_tables::IANA_V6.to_vec());
    }

    #[test]
    fn both_ends_of_each_pinned_range_are_nonordinary() {
        for &(base, prefix) in iana_tables::IANA_V4 {
            let mask = if prefix == 0 {
                0
            } else {
                u32::MAX << (32 - prefix)
            };
            for address in [base, base | !mask] {
                assert_ne!(
                    classify(IpAddr::V4(Ipv4Addr::from(address))),
                    AddressClass::Ordinary
                );
            }
        }
        for &(base, prefix) in iana_tables::IANA_V6 {
            let mask = if prefix == 0 {
                0
            } else {
                u128::MAX << (128 - prefix)
            };
            for address in [base, base | !mask] {
                assert_ne!(classify(IpAddr::V6(address.into())), AddressClass::Ordinary);
            }
        }
    }

    #[test]
    fn special_and_multicast_classes_are_distinct() {
        for value in [
            "0.0.0.0",
            "10.0.0.1",
            "100.64.0.1",
            "169.254.169.254",
            "192.0.0.170",
            "198.18.0.1",
            "203.0.113.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
        ] {
            assert_eq!(classify(ip(value)), AddressClass::IanaSpecial, "{value}");
        }
        for value in ["224.0.0.1", "ff02::1"] {
            assert_eq!(classify(ip(value)), AddressClass::Multicast);
        }
        for value in ["240.0.0.1", "255.255.255.255"] {
            assert_eq!(classify(ip(value)), AddressClass::Reserved);
        }
        for value in ["64:ff9b::1", "64:ff9b:1::1", "2001::1", "2002::1"] {
            assert_eq!(classify(ip(value)), AddressClass::TranslationTunnel);
        }
        assert_eq!(
            classify(ip("::ffff:127.0.0.1")),
            AddressClass::Mapped(Ipv4Addr::new(127, 0, 0, 1))
        );
        // Deprecated compatible forms must not become ordinary destinations.
        for value in [
            "::2",
            "::127.0.0.1",
            "::10.0.0.1",
            "::8.8.8.8",
            "::ffff:ffff",
            "0:0:0:0:0:0:7f00:1",
        ] {
            assert_ne!(
                classify(ip(value)),
                AddressClass::Ordinary,
                "sf06_deprecated_compatible_not_ordinary"
            );
        }
        for value in ["8.8.8.8", "1.1.1.1", "2606:4700:4700::1111"] {
            assert_eq!(classify(ip(value)), AddressClass::Ordinary);
        }
    }

    #[test]
    fn default_deny_is_atomic_and_exception_is_exact() {
        let p = policy(&[], 8);
        assert_eq!(
            check(&p, &[ip("8.8.8.8"), ip("127.0.0.1")]).unwrap_err(),
            AddressError::Disallowed
        );
        let p = policy(&["127.0.0.1"], 8);
        assert!(check(&p, &[ip("8.8.8.8"), ip("127.0.0.1")]).is_ok());
        assert_eq!(
            check(&p, &[ip("127.0.0.2")]).unwrap_err(),
            AddressError::Disallowed
        );
        // Inspect the complete answer: one legacy destination rejects the set.
        for value in [
            "fec0::",
            "fec0::1",
            "feff:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
            "::2",
            "::127.0.0.1",
            "::10.0.0.1",
            "::8.8.8.8",
            "::ffff:ffff",
        ] {
            assert!(
                matches!(
                    check(&policy(&[], 8), &[ip("8.8.8.8"), ip(value)]),
                    Err(AddressError::Disallowed)
                ),
                "sf06_legacy_ipv6_default_deny"
            );
        }
        // The inherited test-network escape stays exact; production has none.
        let legacy = policy(&["fec0::1"], 8);
        assert!(check(&legacy, &[ip("fec0::1")]).is_ok());
        assert_eq!(
            check(&legacy, &[ip("fec0::2")]).unwrap_err(),
            AddressError::Disallowed
        );
    }

    #[test]
    fn mapped_ipv4_needs_both_exact_forms() {
        let mapped = ip("::ffff:127.0.0.1");
        assert_eq!(
            check(&policy(&["::ffff:127.0.0.1"], 8), &[mapped]).unwrap_err(),
            AddressError::Disallowed
        );
        assert_eq!(
            check(&policy(&["127.0.0.1"], 8), &[mapped]).unwrap_err(),
            AddressError::Disallowed
        );
        assert!(check(&policy(&["127.0.0.1", "::ffff:127.0.0.1"], 8), &[mapped]).is_ok());
    }

    #[test]
    fn count_is_checked_before_dedup_or_filtering() {
        let p = policy(&[], 2);
        assert_eq!(
            check(&p, &[ip("8.8.8.8"), ip("8.8.8.8"), ip("8.8.8.8")]).unwrap_err(),
            AddressError::TooManyCandidates
        );
        assert_eq!(check(&p, &[]).unwrap_err(), AddressError::Empty);
    }

    #[test]
    fn accepted_candidates_sort_v4_then_v6_after_full_check() {
        let p = policy(&[], 8);
        let checked = check(
            &p,
            &[
                ip("2606:4700:4700::1111"),
                ip("8.8.8.8"),
                ip("1.1.1.1"),
                ip("8.8.8.8"),
            ],
        )
        .unwrap();
        assert_eq!(checked.observed_count, 4);
        assert_eq!(
            checked.checked_candidates,
            vec![ip("1.1.1.1"), ip("8.8.8.8"), ip("2606:4700:4700::1111")]
        );
        assert_eq!(checked.selected, SocketAddr::new(ip("1.1.1.1"), 443));
    }

    #[test]
    fn policy_and_snapshot_are_bound_to_identity_and_port() {
        let p = policy(&["127.0.0.1"], 8);
        let mut other = ID;
        other.generation += 1;
        let now = Instant::now();
        assert_eq!(
            check_candidates(
                &p,
                other,
                &[ip("127.0.0.1")],
                now + Duration::from_secs(1),
                now + Duration::from_secs(2),
                now
            )
            .unwrap_err(),
            AddressError::IdentityMismatch
        );
        assert_eq!(check(&p, &[ip("127.0.0.1")]).unwrap().selected.port(), 443);
    }

    #[test]
    fn strict_expiry_rejects_zero_ttl_and_late_dial() {
        let p = policy(&[], 8);
        let now = Instant::now();
        let end = now + Duration::from_secs(5);
        assert_eq!(
            check_candidates(&p, ID, &[ip("8.8.8.8")], now, end, now).unwrap_err(),
            AddressError::Expired
        );
        assert_eq!(
            check_candidates(&p, ID, &[ip("8.8.8.8")], end, now, now).unwrap_err(),
            AddressError::Expired
        );
        let checked = check_candidates(&p, ID, &[ip("8.8.8.8")], end, end, now).unwrap();
        assert_eq!(checked.begin_dial(end).unwrap_err(), AddressError::Expired);
        let ttl_end = now + Duration::from_secs(2);
        let checked = check_candidates(&p, ID, &[ip("8.8.8.8")], ttl_end, end, now).unwrap();
        assert_eq!(
            checked.begin_dial(ttl_end).unwrap_err(),
            AddressError::Expired
        );
        let checked = check_candidates(&p, ID, &[ip("8.8.8.8")], ttl_end, end, now).unwrap();
        let selection = checked.begin_dial(now).unwrap();
        assert_eq!(selection.identity, ID);
        assert_eq!(selection.selected, SocketAddr::new(ip("8.8.8.8"), 443));
        assert_eq!(selection.valid_until, ttl_end);
        assert_eq!(selection.operation_end, end);
    }

    #[test]
    fn literal_uses_identical_classification_without_resolver() {
        let p = policy(&[], 8);
        let now = Instant::now();
        let until = now + Duration::from_secs(1);
        assert_eq!(
            check_literal(&p, ID, ip("127.0.0.1"), until, until, now).unwrap_err(),
            AddressError::Disallowed
        );
        assert_eq!(
            check_literal(&p, ID, ip("8.8.8.8"), until, until, now)
                .unwrap()
                .selected
                .ip(),
            ip("8.8.8.8")
        );
    }

    #[test]
    fn policy_limits_are_finite_and_duplicate_exceptions_rejected() {
        assert_eq!(
            InternalPolicy::checked(ID, 0, 8, vec![]).unwrap_err(),
            AddressError::InvalidPolicy
        );
        assert_eq!(
            InternalPolicy::checked(ID, 443, 0, vec![]).unwrap_err(),
            AddressError::InvalidPolicy
        );
        assert_eq!(
            InternalPolicy::checked(ID, 443, usize::MAX, vec![]).unwrap_err(),
            AddressError::InvalidPolicy
        );
        assert_eq!(
            InternalPolicy::checked(ID, 443, 8, vec![ip("127.0.0.1"); 65]).unwrap_err(),
            AddressError::InvalidPolicy
        );
        assert_eq!(
            InternalPolicy::checked(ID, 443, 8, vec![ip("127.0.0.1"); 2]).unwrap_err(),
            AddressError::InvalidPolicy
        );
    }
}
