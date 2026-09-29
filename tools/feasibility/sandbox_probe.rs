use std::{
    fs,
    io::{self, Read},
    net::TcpListener,
    process::Command,
};
fn main() {
    let root = std::env::args().nth(1).expect("probe directory");
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let read_allowed = fs::read_to_string(format!("{root}/outside-sentinel")).is_ok();
    let write_allowed = fs::write(format!("{root}/outside-write"), "owned probe").is_ok();
    let listen_allowed = TcpListener::bind("127.0.0.1:0").is_ok();
    let child_allowed = Command::new("/usr/bin/true")
        .status()
        .is_ok_and(|s| s.success());
    println!(
        "{{\"stdin_ok\":{},\"outside_read_allowed\":{},\"outside_write_allowed\":{},\"loopback_listen_allowed\":{},\"child_allowed\":{}}}",
        input.trim() == "synthetic input",
        read_allowed,
        write_allowed,
        listen_allowed,
        child_allowed
    );
}
