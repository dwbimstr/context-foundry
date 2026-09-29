//! macOS-only scratch launcher. Lower the hard limit before loading model libraries.
use std::os::unix::process::CommandExt;
#[repr(C)]
struct Limit {
    current: u64,
    maximum: u64,
}
unsafe extern "C" {
    fn setrlimit(resource: i32, limit: *const Limit) -> i32;
}
fn main() {
    assert!(cfg!(target_os = "macos"));
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
    let mut args = std::env::args().skip(1);
    let target = args.next().expect("absolute worker path");
    let err = std::process::Command::new(target).args(args).exec();
    panic!("worker exec failed: {err}");
}
