//! A small closed-loop HTTP/1 keep-alive load client for the auth comparison.
//! Compile with rustc -O; no new Cargo dependency or throughput oracle is used.
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    sync::{Arc, Barrier},
    thread,
    time::{Duration, Instant},
};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let port: u16 = args[1].parse().unwrap();
    let workers: usize = args[2].parse().unwrap();
    let seconds: u64 = args[3].parse().unwrap();
    let barrier = Arc::new(Barrier::new(workers + 1));
    let mut tasks = vec![];
    for _ in 0..workers {
        let barrier = barrier.clone();
        tasks.push(thread::spawn(move || {
            let mut socket = TcpStream::connect(("127.0.0.1",port)).unwrap();
            socket.set_nodelay(true).unwrap();
            socket.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            socket.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let request = format!("GET /documents/1 HTTP/1.1\r\nHost: localhost:{port}\r\nAuthorization: Bearer demo-alice\r\nConnection: keep-alive\r\n\r\n");
            let mut line = String::new();
            let mut body = vec![];
            let mut latencies = Vec::with_capacity(100_000);
            let mut errors = [0usize;3]; // transport, status, body
            barrier.wait();
            let deadline = Instant::now() + Duration::from_secs(seconds);
            while Instant::now() < deadline {
                let start = Instant::now();
                let result = (|| -> std::io::Result<(bool,bool)> {
                    socket.write_all(request.as_bytes())?;
                    line.clear();
                    reader.read_line(&mut line)?;
                    let success = line.starts_with("HTTP/1.1 200 ");
                    let mut length = None;
                    let mut headers = 0;
                    loop {
                        line.clear();
                        if reader.read_line(&mut line)? == 0 { return Err(std::io::ErrorKind::UnexpectedEof.into()); }
                        headers += line.len();
                        if headers > 16_384 { return Err(std::io::ErrorKind::InvalidData.into()); }
                        if line == "\r\n" { break; }
                        if let Some((key, value)) = line.split_once(':') {
                            if key.eq_ignore_ascii_case("content-length") { length = value.trim().parse::<usize>().ok(); }
                        }
                    }
                    let length = length.filter(|length| *length <= 16_384).ok_or(std::io::ErrorKind::InvalidData)?;
                    body.resize(length,0);
                    reader.read_exact(&mut body)?;
                    Ok((success,body==br#"{"id":1,"title":"Alice document"}"#))
                })();
                match result {
                    Ok((true,true)) => latencies.push(start.elapsed().as_nanos() as u64),
                    Ok((false,_)) => errors[1] += 1,
                    Ok((true,false)) => errors[2] += 1,
                    Err(_) => { errors[0] += 1; break; },
                }
            }
            (latencies,errors)
        }));
    }
    barrier.wait();
    let start = Instant::now();
    let mut latencies = vec![];
    let mut errors = [0usize; 3];
    for task in tasks {
        let (values, counts) = task.join().unwrap();
        latencies.extend(values);
        for (total, count) in errors.iter_mut().zip(counts) {
            *total += count;
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    latencies.sort_unstable();
    let percentile = |p: f64| {
        latencies
            .get(((latencies.len().saturating_sub(1)) as f64 * p) as usize)
            .copied()
            .unwrap_or(0) as f64
            / 1000.0
    };
    println!("{{\"requests\":{},\"seconds\":{},\"qps\":{},\"p50_us\":{},\"p95_us\":{},\"p99_us\":{},\"transport_errors\":{},\"status_errors\":{},\"body_errors\":{},\"concurrency\":{}}}",latencies.len(),elapsed,latencies.len() as f64/elapsed,percentile(0.5),percentile(0.95),percentile(0.99),errors[0],errors[1],errors[2],workers);
}
