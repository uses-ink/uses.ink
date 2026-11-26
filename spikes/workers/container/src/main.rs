//! Stand-in for `ink build` inside the Container.
//!
//! `emit lines <count> <interval-ms>` prints numbered lines to stdout over time.
//! `emit serve` listens on 8080 and serves the same lines as a chunked HTTP body
//! (`/run`), a readiness probe (`/ping`) and an egress probe (`/egress?host=`).
//! Every line carries `t=<epoch ms>` so a client on the same host can measure latency.

use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{env, thread};

fn epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

fn line(i: u32) -> String {
    format!("line {i} t={}\n", epoch_ms())
}

fn lines(count: u32, interval: u64) -> io::Result<()> {
    let mut out = io::stdout().lock();
    for i in 0..count {
        out.write_all(line(i).as_bytes())?;
        out.flush()?;
        thread::sleep(Duration::from_millis(interval));
    }
    eprintln!("emit: done, {count} lines");
    Ok(())
}

fn query<'a>(target: &'a str, key: &str) -> Option<&'a str> {
    let (_, q) = target.split_once('?')?;
    q.split('&')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v)
}

fn respond(stream: &mut TcpStream, status: &str, body: &str) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn run(stream: &mut TcpStream, count: u32, interval: u64) -> io::Result<()> {
    stream.write_all(
        b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n",
    )?;
    for i in 0..count {
        let l = line(i);
        // The container's own stdout gets the line too, as `ink build` would print it.
        print!("{l}");
        io::stdout().flush()?;
        write!(stream, "{:x}\r\n{l}\r\n", l.len())?;
        stream.flush()?;
        thread::sleep(Duration::from_millis(interval));
    }
    stream.write_all(b"0\r\n\r\n")
}

/// Plain HTTP GET to `host:80`, reporting each step, so a blocked egress says where it stopped.
fn egress(host: &str) -> String {
    let started = Instant::now();
    let addrs = match (host, 80).to_socket_addrs() {
        Ok(a) => a.collect::<Vec<_>>(),
        Err(e) => return format!("{host}: dns failed after {:?}: {e}\n", started.elapsed()),
    };
    let Some(addr) = addrs.first() else {
        return format!("{host}: dns returned nothing\n");
    };
    let mut s = match TcpStream::connect_timeout(addr, Duration::from_secs(3)) {
        Ok(s) => s,
        Err(e) => return format!("{host}: resolved {addrs:?}, connect failed after {:?}: {e}\n", started.elapsed()),
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
    if let Err(e) = write!(s, "GET / HTTP/1.1\r\nhost: {host}\r\nconnection: close\r\n\r\n") {
        return format!("{host}: write failed: {e}\n");
    }
    let mut status = String::new();
    match BufReader::new(s).read_line(&mut status) {
        Ok(_) => format!("{host}: resolved {addrs:?}, answered {:?} in {:?}\n", status.trim(), started.elapsed()),
        Err(e) => format!("{host}: resolved {addrs:?}, read failed after {:?}: {e}\n", started.elapsed()),
    }
}

fn connection(mut stream: TcpStream) -> io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request = String::new();
    reader.read_line(&mut request)?;
    // Drain headers; bodies are not used.
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? <= 2 {
            break;
        }
    }
    let target = request.split_whitespace().nth(1).unwrap_or("/");
    let path = target.split('?').next().unwrap_or("/");
    match path {
        "/ping" => respond(&mut stream, "200 OK", "ok\n"),
        "/run" => {
            let count = query(target, "lines").and_then(|v| v.parse().ok()).unwrap_or(10);
            let interval = query(target, "interval").and_then(|v| v.parse().ok()).unwrap_or(200);
            run(&mut stream, count, interval)
        }
        "/egress" => {
            let host = query(target, "host").unwrap_or("example.com").to_owned();
            respond(&mut stream, "200 OK", &egress(&host))
        }
        _ => respond(&mut stream, "404 Not Found", "no such route\n"),
    }
}

fn serve() -> io::Result<()> {
    println!("emit: process up t={}", epoch_ms());
    let listener = TcpListener::bind("0.0.0.0:8080")?;
    println!("emit: listening t={}", epoch_ms());
    for stream in listener.incoming() {
        let stream = stream?;
        thread::spawn(move || {
            if let Err(e) = connection(stream) {
                eprintln!("emit: connection: {e}");
            }
        });
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("lines") => {
            let count = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(10);
            let interval = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(200);
            lines(count, interval)
        }
        Some("serve") | None => serve(),
        Some(other) => {
            eprintln!("emit: unknown command {other}");
            std::process::exit(2)
        }
    }
}
