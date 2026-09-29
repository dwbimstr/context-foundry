use std::{
    fs,
    io::{self, Read},
    net::TcpListener,
    process::Command,
};
#[repr(C)]
struct Limit {
    current: u64,
    maximum: u64,
}
unsafe extern "C" {
    fn setrlimit(resource: i32, limit: *const Limit) -> i32;
    fn getrlimit(resource: i32, limit: *mut Limit) -> i32;
    fn fork() -> i32;
    fn _exit(status: i32) -> !;
}
fn main() {
    assert_eq!(
        unsafe {
            setrlimit(
                7,
                &Limit {
                    current: 0,
                    maximum: 0,
                },
            )
        },
        0
    );
    let mut l = Limit {
        current: 99,
        maximum: 99,
    };
    assert_eq!(unsafe { getrlimit(7, &mut l) }, 0);
    let root = std::env::args().nth(1).unwrap();
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let child = Command::new("/usr/bin/true").status();
    let child_allowed = child.as_ref().is_ok_and(|s| s.success());
    let fork_pid = unsafe { fork() };
    if fork_pid == 0 {
        unsafe { _exit(0) }
    };
    let raise_allowed = unsafe {
        setrlimit(
            7,
            &Limit {
                current: 1,
                maximum: 1,
            },
        )
    } == 0;
    let thread = std::thread::spawn(|| 42).join().unwrap() == 42;
    println!(
        "{{\"nproc_soft\":{},\"nproc_hard\":{},\"raise_allowed\":{},\"child_allowed\":{},\"fork_allowed\":{},\"threads_work\":{},\"outside_read_allowed\":{},\"outside_write_allowed\":{},\"loopback_listen_allowed\":{}}}",
        l.current,
        l.maximum,
        raise_allowed,
        child_allowed,
        fork_pid >= 0,
        thread,
        fs::read(format!("{root}/outside-sentinel")).is_ok(),
        fs::write(format!("{root}/outside-write"), "probe").is_ok(),
        TcpListener::bind("127.0.0.1:0").is_ok()
    );
    assert!(!child_allowed && fork_pid < 0 && !raise_allowed && thread);
}
