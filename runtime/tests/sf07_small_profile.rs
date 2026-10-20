//! Bounded public-API loopback profile for the adopted SF07 response/rate limits.
//! This records one target's calling-thread allocator counters and wall-clock
//! samples; it is not an RSS, process-wide allocation, or service-level test.
use nagi_runtime::{http_server as http, metrics, Error};
use std::{
    mem::size_of,
    net::SocketAddr,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    runtime::Builder,
    sync::oneshot,
};

const WARMUPS: usize = 4;
const SAMPLES: usize = 32;
const MAX_RESPONSE_BYTES: usize = 16 * 1024;
const HEADER_VALUE: &str = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
const HEADER_NAMES: [&str; 12] = [
    "x-profile-00",
    "x-profile-01",
    "x-profile-02",
    "x-profile-03",
    "x-profile-04",
    "x-profile-05",
    "x-profile-06",
    "x-profile-07",
    "x-profile-08",
    "x-profile-09",
    "x-profile-10",
    "x-profile-11",
];

const BODY_BYTES: i64 = 4096;
const USER_HEADER_BYTES: i64 = 4096;
const USER_HEADER_COUNT: i64 = 16;
const MANAGED_HEADER_BYTES: i64 = 4096;
const MANAGED_HEADER_COUNT: i64 = 32;
const SERIALIZATION_WRITES: i64 = 4096;
const LIVE_RESPONSES: i64 = 8;
const ORDINARY_RETAINED_BYTES: i64 = 2 * 1024 * 1024;
const FALLBACK_RESPONSES: i64 = 8;
const FALLBACK_RETAINED_BYTES: i64 = 2 * 1024 * 1024;
const FALLBACK_MANAGED_HEADER_BYTES: i64 = 4096;
const FALLBACK_MANAGED_HEADER_COUNT: i64 = 32;

const RATE_RULES: i64 = 8;
const RATE_DOMAINS: i64 = 8;
const RATE_SUBJECT_SLOTS: i64 = 1;
const RATE_RETAINED_BYTES: i64 = 1024 * 1024;
const RATE_REQUEST_WORK: i64 = 65_536;
const GENEROUS_BURST: i64 = 1000;
const GENEROUS_REFILL_MS: i64 = 1000;
const LIMITED_BURST: i64 = 1;
const LIMITED_REFILL_MS: i64 = 86_400_000;
const CONNECTION_SLOTS: i64 = 8;
const REQUEST_SLOTS: i64 = 8;

fn with_test_response_limits(options: http::Options) -> http::Options {
    let limits = http::response_limits(
        BODY_BYTES,
        USER_HEADER_BYTES,
        USER_HEADER_COUNT,
        MANAGED_HEADER_BYTES,
        MANAGED_HEADER_COUNT,
        SERIALIZATION_WRITES,
        LIVE_RESPONSES,
        ORDINARY_RETAINED_BYTES,
        FALLBACK_RESPONSES,
        FALLBACK_RETAINED_BYTES,
        FALLBACK_MANAGED_HEADER_BYTES,
        FALLBACK_MANAGED_HEADER_COUNT,
    )
    .unwrap();
    http::with_response_limits(options, limits).unwrap()
}

fn with_test_rate_limits(options: http::Options) -> http::Options {
    let limits = http::rate_limits(
        RATE_RULES,
        RATE_DOMAINS,
        RATE_SUBJECT_SLOTS,
        RATE_RETAINED_BYTES,
        RATE_REQUEST_WORK,
    )
    .unwrap();
    http::with_rate_limits(options, limits).unwrap()
}

fn attach_rate<S, E>(
    app: http::App<S, E>,
    path: &str,
    burst: i64,
    refill_ms: i64,
) -> http::App<S, E> {
    let policy = http::rate_policy(burst, refill_ms).unwrap();
    http::rate_route(app, http::Method::GET, path, policy).unwrap()
}

async fn handler(
    request: http::Request,
    _: std::sync::Arc<()>,
    _: (),
) -> Result<http::Response, Error> {
    let context = http::response_context(&request);
    match request.path() {
        "/ordinary" => http::text(context, http::Status::OK, "ok"),
        "/headers" => {
            let mut response = http::text(context, http::Status::OK, "headers")?;
            for name in HEADER_NAMES {
                response = http::append_header_text(response, name, HEADER_VALUE)?;
            }
            Ok(response)
        }
        "/limited" => http::text(context, http::Status::OK, "limited"),
        "/healthy" => http::text(context, http::Status::OK, "healthy"),
        _ => http::text(context, http::Status::NOT_FOUND, "missing"),
    }
}

struct RunningServer {
    address: SocketAddr,
    stop: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<Result<(), Error>>,
}

async fn start_server() -> RunningServer {
    let mut app = http::app_default(());
    for path in ["/ordinary", "/headers", "/limited", "/healthy"] {
        app = http::route(app, http::Method::GET, path, http::public_policy(), handler).unwrap();
    }
    app = attach_rate(app, "/ordinary", GENEROUS_BURST, GENEROUS_REFILL_MS);
    app = attach_rate(app, "/headers", GENEROUS_BURST, GENEROUS_REFILL_MS);
    app = attach_rate(app, "/limited", LIMITED_BURST, LIMITED_REFILL_MS);
    app = attach_rate(app, "/healthy", GENEROUS_BURST, GENEROUS_REFILL_MS);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let options = http::capacity(http::default_options(), CONNECTION_SLOTS, REQUEST_SLOTS).unwrap();
    let options = with_test_response_limits(options);
    let options = with_test_rate_limits(options);
    let options = http::authority(
        options,
        "https://localhost",
        vec!["localhost".to_owned()],
        1,
        128,
    )
    .unwrap();
    let (stop, stopped) = oneshot::channel();
    let task = tokio::spawn(http::serve_listener(listener, app, options, async move {
        let _ = stopped.await;
    }));
    RunningServer {
        address,
        stop,
        task,
    }
}

fn request_bytes(path: &str) -> Vec<u8> {
    format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").into_bytes()
}

async fn round_trip(address: SocketAddr, request: &[u8]) -> Vec<u8> {
    let mut stream = TcpStream::connect(address).await.unwrap();
    stream.write_all(request).await.unwrap();
    let mut response = Vec::with_capacity(2048);
    let read = tokio::time::timeout(
        Duration::from_secs(10),
        stream
            .take((MAX_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut response),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        read <= MAX_RESPONSE_BYTES,
        "loopback response exceeded capture bound"
    );
    response
}

fn response_view(response: &[u8]) -> (&str, &[u8]) {
    let split = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("complete bounded HTTP response");
    let headers = std::str::from_utf8(&response[..split]).expect("ASCII HTTP headers");
    (headers, &response[split + 4..])
}

fn assert_status_body(response: &[u8], status: u16, body: &[u8]) {
    let (headers, actual_body) = response_view(response);
    let first = headers.lines().next().expect("status line");
    assert!(first.starts_with("HTTP/1.1 "), "HTTP/1.1 response");
    let actual_status: u16 = first
        .split_ascii_whitespace()
        .nth(1)
        .expect("status code")
        .parse()
        .expect("numeric status code");
    assert_eq!(actual_status, status);
    assert_eq!(actual_body, body);
}

fn custom_header_facts(response: &[u8]) -> (usize, usize) {
    let (headers, _) = response_view(response);
    headers
        .lines()
        .skip(1)
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            HEADER_NAMES
                .iter()
                .any(|known| name.eq_ignore_ascii_case(known))
                .then(|| (1, value.trim().len()))
        })
        .fold((0, 0), |(count, bytes), (next_count, next_bytes)| {
            (count + next_count, bytes + next_bytes)
        })
}

fn retry_after(response: &[u8]) -> Option<String> {
    let (headers, _) = response_view(response);
    headers.lines().skip(1).find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("retry-after")
            .then(|| value.trim().to_owned())
    })
}

fn elapsed_ns(started: Instant) -> u64 {
    started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

fn percentile_summary(samples: &[u64]) -> (u64, u64) {
    assert_eq!(samples.len(), SAMPLES);
    let mut ordered = samples.to_vec();
    ordered.sort_unstable();
    let median = ((u128::from(ordered[15]) + u128::from(ordered[16])) / 2) as u64;
    let p95_index = (ordered.len() * 95).div_ceil(100) - 1;
    (median, ordered[p95_index])
}

struct ProfileMeasurements<'a> {
    samples: &'a [u64],
    allocations: metrics::Allocations,
    header_count: usize,
    header_value_bytes: usize,
    header_body_bytes: usize,
    header_latency_ns: u64,
    limited_accept_latency_ns: u64,
    limited_denied_latency_ns: u64,
    healthy_latency_ns: u64,
    retry_after_seconds: u64,
}

fn make_report(measurements: ProfileMeasurements<'_>) -> nagi_runtime::serde_json::Value {
    let ProfileMeasurements {
        samples,
        allocations,
        header_count,
        header_value_bytes,
        header_body_bytes,
        header_latency_ns,
        limited_accept_latency_ns,
        limited_denied_latency_ns,
        healthy_latency_ns,
        retry_after_seconds,
    } = measurements;
    let (median_ns, p95_ns) = percentile_summary(samples);
    nagi_runtime::serde_json::json!({
        "schema": "nagi.sf07.small_profile.v1",
        "platform": std::env::consts::OS,
        "architecture": std::env::consts::ARCH,
        "fixture": {
            "transport": "loopback_http1",
            "route_count": 4,
            "connection_slots": CONNECTION_SLOTS,
            "request_slots": REQUEST_SLOTS,
            "client_concurrency": 1,
            "authority_entry_limit": 1,
            "authority_byte_limit": 128,
            "other_server_options": "default_options; not introspected",
            "response_limits": {
                "body_bytes": BODY_BYTES,
                "user_header_bytes": USER_HEADER_BYTES,
                "user_header_count": USER_HEADER_COUNT,
                "managed_header_bytes": MANAGED_HEADER_BYTES,
                "managed_header_count": MANAGED_HEADER_COUNT,
                "serialization_writes": SERIALIZATION_WRITES,
                "live_responses": LIVE_RESPONSES,
                "retained_bytes": ORDINARY_RETAINED_BYTES,
                "fallback_responses": FALLBACK_RESPONSES,
                "fallback_retained_bytes": FALLBACK_RETAINED_BYTES,
                "fallback_managed_header_bytes": FALLBACK_MANAGED_HEADER_BYTES,
                "fallback_managed_header_count": FALLBACK_MANAGED_HEADER_COUNT
            },
            "rate_limits": {
                "rules": RATE_RULES,
                "domains": RATE_DOMAINS,
                "subject_slots": RATE_SUBJECT_SLOTS,
                "retained_bytes": RATE_RETAINED_BYTES,
                "request_work": RATE_REQUEST_WORK
            },
            "route_policies": [
                {"path":"/ordinary", "burst":GENEROUS_BURST, "refill_ms":GENEROUS_REFILL_MS},
                {"path":"/headers", "burst":GENEROUS_BURST, "refill_ms":GENEROUS_REFILL_MS},
                {"path":"/limited", "burst":LIMITED_BURST, "refill_ms":LIMITED_REFILL_MS},
                {"path":"/healthy", "burst":GENEROUS_BURST, "refill_ms":GENEROUS_REFILL_MS}
            ],
            "header_heavy_value_bytes": HEADER_VALUE.len()
        },
        "type_sizes_bytes": {
            "response_context": size_of::<http::ResponseContext>(),
            "response": size_of::<http::Response>(),
            "response_limits": size_of::<http::ResponseLimits>(),
            "rate_limits": size_of::<http::RateLimits>(),
            "rate_policy": size_of::<http::RatePolicy>()
        },
        "observed": {
            "warmup_requests": WARMUPS,
            "ordinary_requests": samples.len(),
            "ordinary_latency_ns": samples,
            "ordinary_median_ns": median_ns,
            "ordinary_p95_ns": p95_ns,
            "header_heavy_status": 200,
            "header_heavy_custom_header_count": header_count,
            "header_heavy_custom_header_value_bytes": header_value_bytes,
            "header_heavy_body_bytes": header_body_bytes,
            "header_heavy_latency_ns": header_latency_ns,
            "limited_accepted_latency_ns": limited_accept_latency_ns,
            "rate_limited_latency_ns": limited_denied_latency_ns,
            "rate_limited_status": 429,
            "rate_limited_retry_after_seconds": retry_after_seconds,
            "healthy_after_rate_fallback_status": 200,
            "healthy_after_rate_fallback_latency_ns": healthy_latency_ns,
            "fallback_retained_high_water": null,
            "retained_high_water_note": "not exposed by the public API",
            "shutdown_joined": true,
            "allocation_scope": "calling-thread allocation events during the 32-request window; includes loopback client, response parsing and current-thread Tokio tasks; deallocations may include pre-window allocations; excludes other threads and process RSS",
            "calling_thread_allocations": allocations
        }
    })
}

#[test]
fn response_profile_reports_closed_measurements() {
    assert_eq!(HEADER_VALUE.len(), 96);
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    let server = runtime.block_on(start_server());
    let ordinary = request_bytes("/ordinary");
    let headers = request_bytes("/headers");
    let limited = request_bytes("/limited");
    let healthy = request_bytes("/healthy");

    for _ in 0..WARMUPS {
        let response = runtime.block_on(round_trip(server.address, &ordinary));
        assert_status_body(&response, 200, b"ok");
    }

    let mut samples = Vec::with_capacity(SAMPLES);
    let (_, allocations) = metrics::measure(|| {
        runtime.block_on(async {
            for _ in 0..SAMPLES {
                let started = Instant::now();
                let response = round_trip(server.address, &ordinary).await;
                samples.push(elapsed_ns(started));
                assert_status_body(&response, 200, b"ok");
            }
        });
    });

    let started = Instant::now();
    let header_response = runtime.block_on(round_trip(server.address, &headers));
    let header_latency_ns = elapsed_ns(started);
    assert_status_body(&header_response, 200, b"headers");
    let (header_count, header_value_bytes) = custom_header_facts(&header_response);
    assert_eq!(header_count, HEADER_NAMES.len());
    assert_eq!(header_value_bytes, HEADER_NAMES.len() * HEADER_VALUE.len());
    let header_body_bytes = response_view(&header_response).1.len();

    let started = Instant::now();
    let first_limited = runtime.block_on(round_trip(server.address, &limited));
    let limited_accept_latency_ns = elapsed_ns(started);
    assert_status_body(&first_limited, 200, b"limited");
    let started = Instant::now();
    let denied = runtime.block_on(round_trip(server.address, &limited));
    let limited_denied_latency_ns = elapsed_ns(started);
    assert_status_body(&denied, 429, b"too many requests");
    let retry_after = retry_after(&denied).expect("finite-rate refusal has Retry-After");
    let retry_after_seconds: u64 = retry_after.parse().unwrap();
    assert!((1..=(LIMITED_REFILL_MS as u64 / 1000)).contains(&retry_after_seconds));
    let started = Instant::now();
    let healthy_response = runtime.block_on(round_trip(server.address, &healthy));
    let healthy_latency_ns = elapsed_ns(started);
    assert_status_body(&healthy_response, 200, b"healthy");

    server.stop.send(()).unwrap();
    runtime.block_on(server.task).unwrap().unwrap();

    let report = make_report(ProfileMeasurements {
        samples: &samples,
        allocations,
        header_count,
        header_value_bytes,
        header_body_bytes,
        header_latency_ns,
        limited_accept_latency_ns,
        limited_denied_latency_ns,
        healthy_latency_ns,
        retry_after_seconds,
    });
    println!("SF07_PROFILE_JSON={report}");
}
