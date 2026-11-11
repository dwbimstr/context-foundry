//! Real descriptor-lifetime regressions. A forked child can retain the search
//! lock after redb explicitly releases authoritative ownership in its parent.
#![cfg(unix)]

use context_foundry::fault::{self, Action, names};
use context_foundry::{Control, Engine, FoundryError};
use std::cell::RefCell;
use std::path::Path;
use std::time::{Duration, Instant};

struct ForkHold {
    pid: libc::pid_t,
    release: i32,
}

impl ForkHold {
    fn new() -> Self {
        let mut ready = [-1; 2];
        let mut release = [-1; 2];
        assert_eq!(unsafe { libc::pipe(ready.as_mut_ptr()) }, 0);
        assert_eq!(unsafe { libc::pipe(release.as_mut_ptr()) }, 0);
        let pid = unsafe { libc::fork() };
        assert!(pid >= 0);
        if pid == 0 {
            // Only async-signal-safe operations after fork. The timeout makes
            // the child leave even if its parent fails before releasing it.
            unsafe {
                libc::close(ready[0]);
                libc::close(release[1]);
                let byte = 1u8;
                libc::write(ready[1], (&byte as *const u8).cast(), 1);
                let mut fd = libc::pollfd {
                    fd: release[0],
                    events: libc::POLLIN,
                    revents: 0,
                };
                libc::poll(&mut fd, 1, 5000);
                libc::_exit(0);
            }
        }
        unsafe {
            libc::close(ready[1]);
            libc::close(release[0]);
        }
        let held = Self {
            pid,
            release: release[1],
        };
        let mut fd = libc::pollfd {
            fd: ready[0],
            events: libc::POLLIN,
            revents: 0,
        };
        let observed = unsafe { libc::poll(&mut fd, 1, 2000) };
        unsafe {
            libc::close(ready[0]);
        }
        assert_eq!(observed, 1, "child reached the held fork/exec boundary");
        held
    }
}

impl Drop for ForkHold {
    fn drop(&mut self) {
        unsafe {
            let byte = 1u8;
            libc::write(self.release, (&byte as *const u8).cast(), 1);
            libc::close(self.release);
            while libc::waitpid(self.pid, std::ptr::null_mut(), 0) == -1
                && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR)
            {}
        }
    }
}

fn seed(root: &Path) -> Engine {
    let ws = root.join("ws");
    std::fs::create_dir(&ws).unwrap();
    let body = "pub fn lock_witness() {}\n";
    std::fs::write(ws.join("lib.rs"), body).unwrap();
    let mut engine = Engine::initialize(&root.join("store"), &ws).unwrap();
    engine.replace_source("lib.rs", body).unwrap();
    engine.refresh(&Control::unbounded()).unwrap();
    engine
}

#[test]
fn inherited_search_lock_does_not_request_repair_after_owner_closes() {
    let dir = tempfile::tempdir().unwrap();
    let engine = seed(dir.path());
    let held = RefCell::new(Some(ForkHold::new()));
    drop(engine);
    fault::arm(
        names::SEARCH_WRITER_BUSY,
        0,
        Action::Call(Box::new(move |_| {
            // Release only after the real writer-lock attempt has observed the
            // inherited descriptor, not after a scheduling-dependent sleep.
            drop(held.borrow_mut().take());
        })),
    );
    let reopened = Engine::open_existing(&dir.path().join("store"));
    let attempts = fault::reached(names::SEARCH_WRITER_BUSY);
    fault::disarm_all(); // releases/reaps the child even on the pre-repair path
    let engine = reopened.unwrap();
    assert_eq!(engine.status().unwrap().index_state, "ready");
    assert!(attempts > 0, "must exercise real inherited lock contention");
    assert_eq!(engine.pending().unwrap(), 0);
    assert_eq!(engine.search("lock_witness", 8).unwrap().hits.len(), 1);
}

#[test]
fn persistent_search_writer_contention_is_bounded_and_never_repair_required() {
    let dir = tempfile::tempdir().unwrap();
    drop(seed(dir.path()));
    let store = dir.path().join("store");
    let index = tantivy::Index::open_in_dir(store.join("search")).unwrap();
    let writer = index
        .writer_with_num_threads::<tantivy::TantivyDocument>(1, 20_000_000)
        .unwrap();
    let meta_before = std::fs::read(store.join("search/meta.json")).unwrap();
    let start = Instant::now();
    let opened = Engine::open_existing(&store);
    assert!(matches!(opened, Err(FoundryError::StoreBusy)));
    assert!(start.elapsed() < Duration::from_secs(3), "bounded open");
    assert_eq!(
        std::fs::read(store.join("search/meta.json")).unwrap(),
        meta_before
    );
    drop(writer);
    let engine = Engine::open_existing(&store).unwrap();
    assert_eq!(engine.status().unwrap().index_state, "ready");
    assert_eq!(engine.source_revision().unwrap(), 1);
    assert_eq!(engine.pending().unwrap(), 0);
    assert_eq!(engine.search("lock_witness", 8).unwrap().hits.len(), 1);
}
