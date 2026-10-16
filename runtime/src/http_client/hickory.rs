//! Exact Hickory 0.26.3 fixed-service resolver integration.
use super::tokio_io::{TrackedTcp, TrackedUdp};
use super::*;
use hickory_resolver::{
    config::{
        LookupIpStrategy, NameServerConfig, OpportunisticEncryption, ResolveHosts, ResolverConfig,
        ResolverOpts, ServerOrderingStrategy,
    },
    lookup::Lookup,
    net::{
        runtime::{
            iocompat::AsyncIoTokioAsStd, DnsUdpSocket, RuntimeProvider, Spawn,
            TokioRuntimeProvider, TokioTime,
        },
        DnsError, NetError, NoRecords,
    },
    proto::{
        op::ResponseCode,
        rr::{DNSClass, Name, RData, RecordType},
    },
    Resolver,
};
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};
use tokio::sync::oneshot;

// Trusted startup config only. No request URL/host/environment/system resolver.
pub(in super::super) struct ResolverStartup {
    name: Name,
    endpoint: SocketAddr,
}
impl ResolverStartup {
    pub(in super::super) fn checked(
        canonical_host: &str,
        endpoint: SocketAddr,
    ) -> Result<Self, DnsFailure> {
        if canonical_host.is_empty()
            || canonical_host.len() > 253
            || !canonical_host.is_ascii()
            || canonical_host.ends_with('.')
            || canonical_host.bytes().any(|b| b.is_ascii_uppercase())
            || !canonical_host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
            || canonical_host.split('.').any(|label| {
                label.is_empty()
                    || label.len() > 63
                    || label.starts_with('-')
                    || label.ends_with('-')
            })
            || canonical_host.parse::<IpAddr>().is_ok()
            || !matches!(url::Host::parse(canonical_host),
                Ok(url::Host::Domain(name)) if name == canonical_host)
            || !matches!(rustls::pki_types::ServerName::try_from(canonical_host),
                Ok(rustls::pki_types::ServerName::DnsName(name)) if name.as_ref() == canonical_host)
            || endpoint.port() == 0
            || endpoint.ip().is_unspecified()
            || endpoint.ip().is_multicast()
        {
            return Err(DnsFailure::Configuration);
        }
        // Hickory has local/special-name shortcuts. This fixed-service profile
        // rejects them at startup rather than pretending they made two queries.
        if special(canonical_host) {
            return Err(DnsFailure::Configuration);
        }
        let absolute = format!("{canonical_host}.");
        let name = Name::from_ascii(&absolute).map_err(|_| DnsFailure::Configuration)?;
        if !name.is_fqdn() || name.to_ascii() != absolute {
            return Err(DnsFailure::Configuration);
        }
        Ok(Self { name, endpoint })
    }
}
fn special(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    ["localhost", "invalid", "local", "onion", "arpa"]
        .iter()
        .any(|suffix| {
            name == *suffix
                || name
                    .strip_suffix(suffix)
                    .is_some_and(|prefix| prefix.ends_with('.'))
        })
}

#[derive(Clone)]
struct TrackedProvider(DnsPhase);
#[derive(Clone)]
struct TrackedSpawn(DnsPhase);
impl Spawn for TrackedSpawn {
    fn spawn_bg(&mut self, future: impl Future<Output = ()> + Send + 'static) {
        let _ = self.0.spawn(TaskKind::DnsChild, future); // error is sticky owner state
    }
}
impl RuntimeProvider for TrackedProvider {
    type Handle = TrackedSpawn;
    type Timer = TokioTime;
    type Udp = TrackedUdp;
    type Tcp = AsyncIoTokioAsStd<TrackedTcp>;
    fn create_handle(&self) -> Self::Handle {
        TrackedSpawn(self.0.clone())
    }
    fn bind_udp(
        &self,
        local: SocketAddr,
        server: SocketAddr,
    ) -> Pin<Box<dyn Future<Output = io::Result<Self::Udp>> + Send>> {
        let phase = self.0.clone();
        Box::pin(async move {
            if server != phase.0.endpoint
                || !local.ip().is_unspecified()
                || local.is_ipv4() != server.is_ipv4()
            {
                phase.fail();
                return Err(io::Error::other("DNS endpoint mismatch"));
            }
            // Ticket starts only when this lazy future is first polled.
            let ticket = phase.begin_io().map_err(|_| {
                phase.fail();
                io::Error::other("DNS IO admission")
            })?;
            // Delegate only bind/connect, NEVER delegate create_handle/spawn.
            let mut native = TokioRuntimeProvider::new().bind_udp(local, server);
            let socket =
                std::future::poll_fn(|cx| phase.poll_io(|_| native.as_mut().poll(cx))).await?;
            drop(native);
            Ok(TrackedUdp::new(socket, ticket, phase))
        })
    }
    fn connect_tcp(
        &self,
        server: SocketAddr,
        bind: Option<SocketAddr>,
        wait: Option<Duration>,
    ) -> Pin<Box<dyn Future<Output = io::Result<Self::Tcp>> + Send>> {
        let phase = self.0.clone();
        Box::pin(async move {
            if server != phase.0.endpoint
                || bind.is_some_and(|b| !b.ip().is_unspecified() || b.is_ipv4() != server.is_ipv4())
            {
                phase.fail();
                return Err(io::Error::other("DNS endpoint mismatch"));
            }
            let ticket = phase.begin_io().map_err(|_| {
                phase.fail();
                io::Error::other("DNS IO admission")
            })?;
            let remaining = phase
                .0
                .end
                .checked_duration_since(Instant::now())
                .ok_or_else(|| io::Error::other("DNS expiry"))?;
            let mut native = TokioRuntimeProvider::new().connect_tcp(
                server,
                bind,
                Some(wait.map_or(remaining, |t| t.min(remaining))),
            );
            let socket =
                std::future::poll_fn(|cx| phase.poll_io(|_| native.as_mut().poll(cx))).await?;
            drop(native);
            if socket.0.peer_addr()? != server {
                drop(socket);
                phase.fail();
                return Err(io::Error::other("DNS peer mismatch"));
            }
            Ok(AsyncIoTokioAsStd(TrackedTcp::new(socket.0, ticket, phase)))
        })
    }
}
impl DnsUdpSocket for TrackedUdp {
    type Time = TokioTime;
    fn poll_recv_from(
        &self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<(usize, SocketAddr)>> {
        TrackedUdp::poll_recv_from(self, cx, buf)
    }
    fn poll_send_to(
        &self,
        cx: &mut Context<'_>,
        buf: &[u8],
        target: SocketAddr,
    ) -> Poll<io::Result<usize>> {
        TrackedUdp::poll_send_to(self, cx, buf, target)
    }
}

fn build(
    startup: &ResolverStartup,
    phase: DnsPhase,
) -> Result<Resolver<TrackedProvider>, DnsFailure> {
    let mut server = NameServerConfig::udp_and_tcp(startup.endpoint.ip());
    for connection in &mut server.connections {
        connection.port = startup.endpoint.port();
        connection.bind_addr = None;
    }
    let mut builder = Resolver::builder_with_config(
        ResolverConfig::from_parts(None, vec![], vec![server]),
        TrackedProvider(phase.clone()),
    );
    let remaining = phase
        .0
        .end
        .checked_duration_since(Instant::now())
        .ok_or(LedgerError::Expired)?;
    let mut options = ResolverOpts::default();
    options.ndots = 1;
    options.timeout = remaining;
    // In 0.26.3 this counts ADDITIONAL attempts after the initial send.
    // Zero retries therefore implements the adopted one logical attempt.
    options.attempts = 0;
    options.edns0 = true;
    options.edns_payload_len = 1232;
    options.ip_strategy = LookupIpStrategy::Ipv4AndIpv6;
    options.cache_size = 16;
    options.use_hosts_file = ResolveHosts::Never;
    options.positive_min_ttl = None;
    options.negative_min_ttl = None;
    options.positive_max_ttl = Some(remaining);
    options.negative_max_ttl = Some(remaining);
    options.num_concurrent_reqs = 1;
    options.max_active_requests = 2;
    options.preserve_intermediates = true;
    options.try_tcp_on_error = false;
    options.server_ordering_strategy = ServerOrderingStrategy::UserProvidedOrder;
    options.recursion_desired = true;
    options.os_port_selection = false;
    options.case_randomization = false;
    options.allow_answers.clear();
    options.deny_answers.clear();
    options.trust_anchor = None;
    *builder.options_mut() = options;
    builder = builder.with_opportunistic_encryption(OpportunisticEncryption::Disabled);
    // No system builder, encryption/persistence builder, global resolver/cache.
    builder.build().map_err(|_| DnsFailure::Resolver)
}

pub(in super::super) enum FamilyOutcome {
    Answers {
        lookup: Lookup,
        addresses: Vec<IpAddr>,
        // Conservative chain bound from the unchanged phase start, never the
        // final response's receipt time. TTL zero remains zero for later policy.
        positive_until: Instant,
    },
    NoData {
        negative: NoRecords,
    },
}
// Constructor is this module only. This is a drained DNS result, NOT the
// address classifier's CheckedSnapshot or authority to dial a destination.
pub(in super::super) struct DrainedResolution {
    name: Name,
    a: FamilyOutcome,
    aaaa: FamilyOutcome,
    end: Instant,
}
impl DrainedResolution {
    pub(in super::super) fn families(&self) -> (&Name, &FamilyOutcome, &FamilyOutcome, Instant) {
        (&self.name, &self.a, &self.aaaa, self.end)
    }
}
fn classify(
    name: &Name,
    kind: RecordType,
    result: Result<Lookup, NetError>,
    started: Instant,
) -> Result<FamilyOutcome, DnsFailure> {
    match result {
        Err(NetError::Dns(DnsError::NoRecordsFound(no)))
            if no.query.name() == name
                && no.query.query_type() == kind
                && no.query.query_class() == DNSClass::IN
                && no.response_code == ResponseCode::NoError =>
        {
            let mut metadata = usize::from(no.soa.is_some());
            metadata = metadata
                .checked_add(no.authorities.as_ref().map_or(0, |a| a.len()))
                .ok_or(DnsFailure::Capacity)?;
            if let Some(servers) = &no.ns {
                for server in servers.iter() {
                    metadata = metadata
                        .checked_add(1)
                        .and_then(|n| n.checked_add(server.glue.len()))
                        .ok_or(DnsFailure::Capacity)?;
                }
            }
            if metadata > RECORDS {
                return Err(DnsFailure::Capacity);
            }
            Ok(FamilyOutcome::NoData { negative: no })
        }
        Err(NetError::Dns(DnsError::NoRecordsFound(no))) => {
            if no.query.name() != name
                || no.query.query_type() != kind
                || no.query.query_class() != DNSClass::IN
            {
                Err(DnsFailure::Answer)
            } else {
                Err(DnsFailure::ResponseCode(no.response_code.into()))
            }
        }
        Err(NetError::Dns(DnsError::ResponseCode(code))) => {
            Err(DnsFailure::ResponseCode(code.into()))
        }
        Err(_) => Err(DnsFailure::Resolver),
        Ok(lookup) => {
            if lookup.message().queries.len() != 1
                || lookup.query().query_type() != kind
                || lookup.query().query_class() != DNSClass::IN
                || lookup.message().metadata.response_code != ResponseCode::NoError
                || lookup.message().metadata.truncation
                || lookup.message().all_sections().count() > RECORDS
            {
                return Err(DnsFailure::Answer);
            }
            let mut final_name = name.clone();
            if lookup
                .answers()
                .iter()
                .any(|r| matches!(&r.data, RData::CNAME(_)) && r.dns_class != DNSClass::IN)
            {
                return Err(DnsFailure::Answer);
            }
            let mut seen = vec![name.clone()];
            // Finite exact chain, with ambiguous/looped/unrelated CNAME rejection.
            for _ in 0..CNAME_HOPS {
                let aliases: Vec<_> = lookup
                    .answers()
                    .iter()
                    .filter(|record| record.name == final_name)
                    .filter_map(|record| {
                        if let RData::CNAME(cname) = &record.data {
                            Some(&cname.0)
                        } else {
                            None
                        }
                    })
                    .collect();
                if aliases.is_empty() {
                    break;
                }
                if aliases.len() != 1
                    || seen.contains(aliases[0])
                    || special(aliases[0].to_ascii().trim_end_matches('.'))
                {
                    return Err(DnsFailure::Answer);
                }
                final_name = aliases[0].clone();
                seen.push(final_name.clone());
            }
            if lookup.answers().iter().any(|r| {
                matches!(&r.data, RData::CNAME(_))
                    && (!seen.contains(&r.name) || r.name == final_name)
            }) {
                return Err(DnsFailure::Answer);
            }
            // Hickory cross-response CNAME lookup returns the terminal query.
            // The initial name is captured by this exact lookup future; accept
            // only it or the final name proven by its preserved exact chain.
            if lookup.query().name() != name
                && (seen.len() == 1 || lookup.query().name() != &final_name)
            {
                return Err(DnsFailure::Answer);
            }
            let mut positive_until = lookup.valid_until();
            for record in lookup.answers() {
                if matches!(&record.data, RData::CNAME(_) | RData::A(_) | RData::AAAA(_)) {
                    let bound = started
                        .checked_add(Duration::from_secs(u64::from(record.ttl)))
                        .ok_or(DnsFailure::Capacity)?;
                    positive_until = positive_until.min(bound);
                }
            }
            let mut addresses = Vec::new();
            addresses
                .try_reserve_exact(RECORDS)
                .map_err(|_| DnsFailure::Capacity)?;
            for record in lookup.answers() {
                match &record.data {
                    RData::A(a)
                        if kind == RecordType::A
                            && record.name == final_name
                            && record.dns_class == DNSClass::IN =>
                    {
                        addresses.push(IpAddr::V4(a.0))
                    }
                    RData::AAAA(a)
                        if kind == RecordType::AAAA
                            && record.name == final_name
                            && record.dns_class == DNSClass::IN =>
                    {
                        addresses.push(IpAddr::V6(a.0))
                    }
                    RData::A(_) | RData::AAAA(_) => return Err(DnsFailure::Answer),
                    _ => {}
                }
            }
            if addresses.is_empty() {
                return Err(DnsFailure::Answer);
            }
            Ok(FamilyOutcome::Answers {
                lookup,
                addresses,
                positive_until,
            })
        }
    }
}

pub(in super::super) async fn resolve(
    runner: &RunnerOwnerHandle,
    key: &JobKey,
    startup: &ResolverStartup,
    resolve_end: Instant,
) -> Result<DrainedResolution, DnsFailure> {
    let phase = DnsPhase::for_job(runner, key, resolve_end, startup.endpoint)?;
    let guard = StopOnDrop(phase.clone());
    let prepared = build(startup, phase.clone());
    let result = match prepared {
        Err(error) => Err(error),
        Ok(resolver) => {
            let name = startup.name.clone();
            let query_name = name.clone();
            let (tx, rx) = oneshot::channel();
            match phase.spawn(TaskKind::DnsRoot, async move {
                // Both owned lookups finish under one original end. No lookup_ip
                // family merging, detached tasks, early-success selection.
                let (a, aaaa) = tokio::join!(
                    resolver.lookup(query_name.clone(), RecordType::A),
                    resolver.lookup(query_name, RecordType::AAAA)
                );
                drop(resolver);
                let _ = tx.send((a, aaaa));
            }) {
                Err(error) => Err(error),
                Ok(()) => match rx.await {
                    Err(_) => Err(DnsFailure::Channel),
                    Ok((a, aaaa)) => match (
                        classify(&name, RecordType::A, a, phase.0.started),
                        classify(&name, RecordType::AAAA, aaaa, phase.0.started),
                    ) {
                        (Ok(a), Ok(aaaa)) => {
                            let count = match &a {
                                FamilyOutcome::Answers { addresses, .. } => addresses.len(),
                                _ => 0,
                            } + match &aaaa {
                                FamilyOutcome::Answers { addresses, .. } => addresses.len(),
                                _ => 0,
                            };
                            if count == 0 {
                                Err(DnsFailure::Empty)
                            } else if count > RECORDS {
                                Err(DnsFailure::Capacity)
                            } else {
                                Ok(DrainedResolution {
                                    name,
                                    a,
                                    aaaa,
                                    end: phase.0.end,
                                })
                            }
                        }
                        (Err(error), _) | (_, Err(error)) => Err(error),
                    },
                },
            }
        }
    };
    // Private result stays withheld until actual DNS joins and raw IO closure.
    // The same stop/drain runs on normal/error paths; dropped waiters only stop.
    let drained = phase.finish().await;
    drop(guard);
    drained?;
    result
}
#[cfg(test)]
#[path = "hickory_tests.rs"]
mod tests;
