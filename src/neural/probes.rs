//! Named development-isolation probes shared by the sandboxed workers
//! (009's `foundry-embed` and 013's `foundry-learn`), moved here from the
//! embedding worker so both bundles run the same checks. Each probe prints
//! one JSON line and exits 0; the verdict (`allowed`, `errno`) is the
//! evidence. Positive controls use the same probe names with granted paths
//! or the unsandboxed binary. A denial only counts as enforcement when its
//! `errno` is the sandbox's or the resource limit's, never a setup failure
//! such as `ENOENT`. No probe loads a model.
use std::io::{Error, Write};
use std::net::{TcpListener, ToSocketAddrs, UdpSocket};

/// Print one verdict line.
pub fn report(check: &str, allowed: bool, detail: String, errno: i32) {
    println!(
        "{{\"check\": {check:?}, \"allowed\": {allowed}, \"detail\": {detail:?}, \"errno\": {errno}}}"
    );
    let _ = std::io::stdout().flush();
}

fn errno(error: &Error) -> i32 {
    error.raw_os_error().unwrap_or(0)
}

/// Open `path` for reading (following symlinks, as any reader would).
fn read(path: &str) {
    match std::fs::File::open(std::path::Path::new(path)) {
        Ok(_) => report("read", true, path.into(), 0),
        Err(e) => report("read", false, path.into(), errno(&e)),
    }
}

/// Open `path` for writing without creating or truncating it, and write
/// one byte at its end: through a symlink this writes the link's target.
fn write(path: &str) {
    match std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(std::path::Path::new(path))
        .and_then(|mut file| file.write_all(b"x"))
    {
        Ok(()) => report("write", true, path.into(), 0),
        Err(e) => report("write", false, path.into(), errno(&e)),
    }
}

fn fds() {
    let mut open = Vec::new();
    for fd in 3..=1024 {
        if unsafe { libc::fcntl(fd, libc::F_GETFD) } != -1 {
            open.push(fd);
        }
    }
    report("fds", true, format!("{open:?}"), 0);
}

fn env() {
    let mut names: Vec<String> = std::env::vars().map(|(k, _)| k).collect();
    names.sort();
    report(
        "env",
        true,
        serde_json::to_string(&names).unwrap_or_default(),
        0,
    );
}

/// Parse `IP:PORT` or `[IPv6]:PORT`; a bad address is a probe failure,
/// reported as not allowed with errno 22 (`EINVAL`), which no denial
/// check accepts.
fn socket_addr(check: &str, target: &str) -> Option<std::net::SocketAddr> {
    match target.parse() {
        Ok(addr) => Some(addr),
        Err(e) => {
            report(check, false, format!("{target}: bad address ({e})"), 22);
            None
        }
    }
}

fn tcp_connect(target: &str) {
    let Some(addr) = socket_addr("tcp-connect", target) else {
        return;
    };
    match std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(3)) {
        Ok(_) => report("tcp-connect", true, target.into(), 0),
        Err(e) => report("tcp-connect", false, target.into(), errno(&e)),
    }
}

fn tcp_listen(target: &str) {
    let Some(addr) = socket_addr("tcp-listen", target) else {
        return;
    };
    match TcpListener::bind(addr) {
        Ok(_) => report("tcp-listen", true, target.into(), 0),
        Err(e) => report("tcp-listen", false, target.into(), errno(&e)),
    }
}

/// Send one datagram to `target` from an ephemeral socket of the same
/// address family.
fn udp_send(target: &str) {
    let Some(addr) = socket_addr("udp-send", target) else {
        return;
    };
    let local = if addr.is_ipv6() {
        "[::1]:0"
    } else {
        "127.0.0.1:0"
    };
    let outcome = UdpSocket::bind(local).and_then(|socket| socket.send_to(b"x", addr));
    match outcome {
        Ok(_) => report("udp-send", true, target.into(), 0),
        Err(e) => report("udp-send", false, target.into(), errno(&e)),
    }
}

/// Listen: bind a UDP socket at `target`.
fn udp_bind(target: &str) {
    let Some(addr) = socket_addr("udp-bind", target) else {
        return;
    };
    match UdpSocket::bind(addr) {
        Ok(_) => report("udp-bind", true, target.into(), 0),
        Err(e) => report("udp-bind", false, target.into(), errno(&e)),
    }
}

fn dns(host: &str) {
    let outcome = (host, 0)
        .to_socket_addrs()
        .map(|resolved| resolved.filter(|addr| addr.is_ipv4()).count());
    match outcome {
        Ok(count) => report("dns", true, format!("{host} resolved {count} addresses"), 0),
        Err(e) => report("dns", false, format!("{host}: {e}"), errno(&e)),
    }
}

fn fork() {
    let pid = unsafe { libc::fork() };
    // Read errno at once: the `waitpid` below overwrites it (ECHILD).
    let fork_errno = if pid < 0 {
        errno(&Error::last_os_error())
    } else {
        0
    };
    if pid == 0 {
        unsafe { libc::_exit(99) };
    }
    let mut status = 0;
    let waited = unsafe { libc::waitpid(pid, &mut status, 0) };
    let code = fork_errno;
    let detail = if waited == pid && (status & 0x7f) == 99 {
        "child ran to _exit(99)".to_string()
    } else if pid < 0 {
        "fork refused".to_string()
    } else {
        format!("fork returned {pid}, waitpid {waited} status {status:#x}")
    };
    report("fork", pid > 0, detail, code);
}

fn spawn() {
    let outcome = std::process::Command::new("/usr/bin/true").status();
    match outcome {
        Ok(status) => report(
            "spawn",
            true,
            format!("exit {}", status.code().unwrap_or(-1)),
            0,
        ),
        Err(e) => report("spawn", false, "spawn refused".into(), errno(&e)),
    }
}

fn nproc_restore() {
    let limit = libc::rlimit {
        rlim_cur: 1,
        rlim_max: 1,
    };
    let rc = unsafe { libc::setrlimit(libc::RLIMIT_NPROC, &limit) };
    if rc == 0 {
        report("nproc-restore", true, "raised NPROC to 1".into(), 0);
    } else {
        report(
            "nproc-restore",
            false,
            "raising NPROC refused".into(),
            errno(&Error::last_os_error()),
        );
    }
}

/// Run one shared probe by name with its arguments.
pub fn run(name: &str, rest: &[String]) {
    match name {
        "read" => read(rest.first().map(String::as_str).unwrap_or("/etc/hosts")),
        "write" => write(
            rest.first()
                .map(String::as_str)
                .unwrap_or("/tmp/cf-embed-probe"),
        ),
        "fds" => fds(),
        "env" => env(),
        "tcp-connect" => tcp_connect(rest.first().map(String::as_str).unwrap_or("127.0.0.1:9")),
        "tcp-listen" => tcp_listen(rest.first().map(String::as_str).unwrap_or("127.0.0.1:0")),
        "udp-send" => udp_send(rest.first().map(String::as_str).unwrap_or("127.0.0.1:9")),
        "udp-bind" => udp_bind(rest.first().map(String::as_str).unwrap_or("127.0.0.1:0")),
        "dns" => dns(rest.first().map(String::as_str).unwrap_or("example.com")),
        "fork" => fork(),
        "spawn" => spawn(),
        "nproc-restore" => nproc_restore(),
        other => report("unknown-probe", false, other.into(), 22),
    }
    let _ = std::io::stdout().flush();
}
