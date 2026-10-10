//! Opt-in owner-local memory admission. Reservations are configured worker ceilings,
//! not measured allocations or an OS quota. Status never probes the host.
use crate::{FResult, FoundryError};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub resident_bytes: u64,
    pub reserve_bytes: u64,
    pub resume_margin_bytes: u64,
}
impl Config {
    pub fn load(path: &Path) -> FResult<Self> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(65537)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 65536 {
            return Err(invalid());
        }
        let config: Self = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
        config.validate()?;
        Ok(config)
    }
    fn validate(&self) -> FResult<()> {
        if self.resident_bytes == 0
            || self.reserve_bytes == 0
            || self.resume_margin_bytes == 0
            || self
                .reserve_bytes
                .checked_add(self.resume_margin_bytes)
                .and_then(|n| n.checked_add(self.resident_bytes))
                .is_none()
        {
            return Err(invalid());
        }
        Ok(())
    }
}
fn invalid() -> FoundryError {
    FoundryError::InvalidArgument(
        "invalid model resource policy: positive, nonoverflowing byte limits required".into(),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lane {
    #[cfg_attr(not(feature = "semantic"), allow(dead_code))]
    Semantic,
    Generation,
}
impl Lane {
    fn index(self) -> usize {
        match self {
            Self::Semantic => 0,
            Self::Generation => 1,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Deferred {
    Budget,
    Pressure,
    Headroom,
    Unavailable,
}
impl Deferred {
    pub fn code(self) -> &'static str {
        match self {
            Self::Budget => "model_memory_budget",
            Self::Pressure => "model_memory_pressure",
            Self::Headroom => "model_memory_headroom",
            Self::Unavailable => "model_memory_observation_unavailable",
        }
    }
}
#[derive(Default)]
struct State {
    reserved: [u64; 2],
    sampled: Option<Instant>,
    refresh: bool,
    available: Option<u64>,
    pressure: bool,
    reason: Option<Deferred>,
}
pub(crate) struct Resources {
    config: Config,
    state: Mutex<State>,
    sample: Box<dyn Fn() -> Option<u64> + Send + Sync>,
}
impl Resources {
    pub(crate) fn new(config: Config) -> FResult<Arc<Self>> {
        Self::with_sampler(config, Box::new(available_memory))
    }
    fn with_sampler(
        config: Config,
        sample: Box<dyn Fn() -> Option<u64> + Send + Sync>,
    ) -> FResult<Arc<Self>> {
        config.validate()?;
        Ok(Arc::new(Self {
            config,
            state: Mutex::new(State::default()),
            sample,
        }))
    }
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    fn check(&self, state: &mut State, additional: u64) -> Result<(), Deferred> {
        if state.refresh
            || state
                .sampled
                .is_none_or(|at| at.elapsed() >= Duration::from_secs(1))
        {
            state.available = (self.sample)();
            state.sampled = Some(Instant::now());
            state.refresh = false;
        }
        let available = state.available.ok_or(Deferred::Unavailable)?;
        let floor = self.config.reserve_bytes
            + if state.pressure {
                self.config.resume_margin_bytes
            } else {
                0
            };
        if available < floor {
            state.pressure = true;
            return Err(Deferred::Pressure);
        }
        state.pressure = false;
        if available.saturating_sub(self.config.reserve_bytes) < additional {
            return Err(Deferred::Headroom);
        }
        Ok(())
    }
    pub(crate) fn background(&self) -> Result<(), Deferred> {
        let mut state = self.lock();
        let result = self.check(&mut state, 0);
        state.reason = result.err();
        result
    }
    pub(crate) fn reserve(
        self: &Arc<Self>,
        lane: Lane,
        bytes: u64,
    ) -> Result<Reservation, Deferred> {
        let mut state = self.lock();
        let total = state.reserved.iter().sum::<u64>();
        let result = if bytes == 0
            || state.reserved[lane.index()] != 0
            || bytes > self.config.resident_bytes.saturating_sub(total)
        {
            Err(Deferred::Budget)
        } else {
            self.check(&mut state, bytes)
        };
        state.reason = result.err();
        result?;
        state.reserved[lane.index()] = bytes;
        // A subsequent load must observe fresh headroom, including this worker.
        state.refresh = true;
        Ok(Reservation {
            resources: self.clone(),
            lane,
        })
    }
    pub(crate) fn status(&self) -> serde_json::Value {
        let state = self.lock();
        serde_json::json!({"policy":self.config,"reserved_semantic_bytes":state.reserved[0],
            "reserved_generation_bytes":state.reserved[1],"available_bytes":state.available,
            "observation_age_ms":state.sampled.map(|at| at.elapsed().as_millis().min(u64::MAX as u128) as u64),
            "reason":state.reason.map(Deferred::code),"pressure":state.pressure,
            "scope":"embedding_and_generation_worker_ceilings", "sample_interval_ms":1000})
    }
    #[cfg(test)]
    pub(crate) fn fixture(
        config: Config,
        available: Arc<std::sync::atomic::AtomicU64>,
    ) -> Arc<Self> {
        Self::with_sampler(
            config,
            Box::new(move || Some(available.load(std::sync::atomic::Ordering::SeqCst))),
        )
        .unwrap()
    }
    #[cfg(test)]
    pub(crate) fn refresh(&self) {
        self.lock().refresh = true;
    }
}
pub(crate) struct Reservation {
    resources: Arc<Resources>,
    lane: Lane,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        let mut state = self.resources.lock();
        state.reserved[self.lane.index()] = 0;
        state.refresh = true;
    }
}

// Missing/malformed observations fail closed. No subprocess, model/store scans or
// cgroup mutation. Container limits are headroom, not permission to create a jail.
#[cfg(target_os = "linux")]
fn available_memory() -> Option<u64> {
    let info = bounded_text(Path::new("/proc/meminfo"))?;
    let available = mem_available(&info)?;
    let membership = bounded_text(Path::new("/proc/self/cgroup"))?;
    let mounts = bounded_text(Path::new("/proc/self/mountinfo"))?;
    let (mount, current) = cgroup_location(&membership, &mounts)?;
    cgroup_headroom(available, &mount, &current)
}
#[cfg(target_os = "linux")]
fn bounded_text(path: &Path) -> Option<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(65537)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 65536 {
        return None;
    }
    String::from_utf8(bytes).ok()
}
#[cfg(target_os = "linux")]
fn mem_available(text: &str) -> Option<u64> {
    let mut fields = text
        .lines()
        .find(|line| line.starts_with("MemAvailable:"))?
        .split_whitespace();
    fields.next()?;
    let value = fields.next()?.parse::<u64>().ok()?.checked_mul(1024)?;
    (fields.next()? == "kB" && fields.next().is_none()).then_some(value)
}
#[cfg(target_os = "linux")]
fn cgroup_location(
    membership: &str,
    mounts: &str,
) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let member = Path::new(
        membership
            .lines()
            .find_map(|line| line.strip_prefix("0::"))?,
    );
    let safe = |path: &Path| {
        path.is_absolute()
            && path.components().all(|c| {
                matches!(
                    c,
                    std::path::Component::RootDir | std::path::Component::Normal(_)
                )
            })
    };
    if !safe(member) {
        return None;
    }
    for line in mounts.lines() {
        let (left, right) = line.split_once(" - ")?;
        if right.split_whitespace().next() != Some("cgroup2") {
            continue;
        }
        let fields: Vec<_> = left.split_whitespace().collect();
        // Linux mountinfo's four defined escapes; reject other escapes.
        let decode = |s: &str| -> Option<std::path::PathBuf> {
            let value = s
                .replace("\\040", " ")
                .replace("\\011", "\t")
                .replace("\\012", "\n");
            if value.replace("\\134", "").contains('\\') {
                return None;
            }
            Some(value.replace("\\134", "\\").into())
        };
        let root = decode(fields.get(3)?)?;
        let mount = decode(fields.get(4)?)?;
        if !safe(&mount) {
            return None;
        }
        // A mount inherited from outside the current cgroup namespace may
        // expose its root as /.. (or more ancestors). Membership '/' then
        // does NOT identify the mount root. Locate this live owner by PID;
        // never erase '..' and silently miss the actual leaf's limits.
        if root.is_absolute()
            && root
                .components()
                .any(|c| c == std::path::Component::ParentDir)
            && root.components().all(|c| {
                matches!(
                    c,
                    std::path::Component::RootDir | std::path::Component::ParentDir
                )
            })
        {
            let current = find_process_group(&mount)?;
            return Some((mount, current));
        }
        if !safe(&root) {
            return None;
        }
        if let Ok(relative) = member.strip_prefix(root) {
            return Some((mount.clone(), mount.join(relative)));
        }
    }
    None // cgroup v1 or an inaccessible namespace is explicitly unsupported.
}
#[cfg(target_os = "linux")]
fn find_process_group(mount: &Path) -> Option<std::path::PathBuf> {
    let pid = std::process::id().to_string();
    let mut paths = vec![(mount.to_owned(), 0)];
    let mut visited = 0;
    while let Some((path, depth)) = paths.pop() {
        visited += 1;
        if visited > 128 || depth > 64 {
            return None;
        }
        if bounded_text(&path.join("cgroup.procs"))
            .is_some_and(|text| text.lines().any(|line| line == pid))
        {
            return Some(path);
        }
        for entry in std::fs::read_dir(&path).ok()? {
            let entry = entry.ok()?;
            if entry.file_type().ok()?.is_dir() {
                if paths.len() + visited >= 128 {
                    return None;
                }
                paths.push((entry.path(), depth + 1));
            }
        }
    }
    None
}
#[cfg(target_os = "linux")]
fn cgroup_headroom(mut available: u64, mount: &Path, current: &Path) -> Option<u64> {
    if !current.starts_with(mount) {
        return None;
    }
    for (depth, path) in current.ancestors().enumerate() {
        if depth > 64 {
            return None;
        }
        for name in ["memory.max", "memory.high"] {
            let limit = match bounded_text(&path.join(name)) {
                Some(text) if text.trim() == "max" => continue,
                Some(text) => text.trim().parse::<u64>().ok()?,
                // The v2 hierarchy root has no limits. A missing descendant
                // interface is unknown, not unlimited.
                None if path == mount
                    && std::fs::symlink_metadata(path.join(name))
                        .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
                {
                    continue;
                }
                None => return None,
            };
            let used = bounded_text(&path.join("memory.current"))?
                .trim()
                .parse::<u64>()
                .ok()?;
            available = available.min(limit.saturating_sub(used));
        }
        if path == mount {
            return Some(available);
        }
    }
    None
}
#[cfg(target_os = "macos")]
fn available_memory() -> Option<u64> {
    // Conservative immediately free pages. Do not promise that inactive or
    // compressed pages can be reclaimed. Unified GPU allocations affect this too.
    unsafe {
        let host = libc::mach_host_self();
        let mut stats: libc::vm_statistics64 = std::mem::zeroed();
        let mut count = libc::HOST_VM_INFO64_COUNT;
        let result = libc::host_statistics64(
            host,
            libc::HOST_VM_INFO64,
            &mut stats as *mut _ as *mut libc::integer_t,
            &mut count,
        );
        libc::mach_port_deallocate(libc::mach_task_self(), host);
        let page = libc::sysconf(libc::_SC_PAGESIZE);
        if result != libc::KERN_SUCCESS || count != libc::HOST_VM_INFO64_COUNT || page <= 0 {
            return None;
        }
        u64::from(stats.free_count).checked_mul(page as u64)
    }
}
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn available_memory() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    fn config() -> Config {
        Config {
            resident_bytes: 1000,
            reserve_bytes: 100,
            resume_margin_bytes: 50,
        }
    }
    #[test]
    fn aggregate_ceiling_is_held_until_residency_ends() {
        let memory = Arc::new(AtomicU64::new(2000));
        let resources = Resources::fixture(config(), memory);
        let embedding = resources.reserve(Lane::Semantic, 700).unwrap();
        assert!(matches!(
            resources.reserve(Lane::Generation, 400),
            Err(Deferred::Budget)
        ));
        let generation = resources.reserve(Lane::Generation, 300).unwrap();
        assert_eq!(resources.status()["reserved_generation_bytes"], 300);
        drop(generation);
        assert_eq!(resources.status()["reserved_generation_bytes"], 0);
        drop(embedding);
        drop(resources.reserve(Lane::Generation, 1000).unwrap());
    }
    #[test]
    fn pressure_has_hysteresis_and_load_requires_additional_headroom() {
        let memory = Arc::new(AtomicU64::new(99));
        let resources = Resources::fixture(config(), memory.clone());
        assert_eq!(resources.background(), Err(Deferred::Pressure));
        memory.store(149, Ordering::SeqCst);
        resources.refresh();
        assert_eq!(resources.background(), Err(Deferred::Pressure));
        memory.store(150, Ordering::SeqCst);
        resources.refresh();
        assert_eq!(resources.background(), Ok(()));
        assert!(matches!(
            resources.reserve(Lane::Generation, 51),
            Err(Deferred::Headroom)
        ));
        drop(resources.reserve(Lane::Generation, 50).unwrap());
    }
    #[test]
    fn unknown_observation_refuses_and_status_does_not_probe() {
        let calls = Arc::new(AtomicU64::new(0));
        let sampled = calls.clone();
        let resources = Resources::with_sampler(
            config(),
            Box::new(move || {
                sampled.fetch_add(1, Ordering::SeqCst);
                None
            }),
        )
        .unwrap();
        assert_eq!(resources.background(), Err(Deferred::Unavailable));
        for _ in 0..20 {
            resources.status();
            resources.background().unwrap_err();
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(matches!(
            resources.reserve(Lane::Semantic, 1),
            Err(Deferred::Unavailable)
        ));
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_observation_honors_nested_limits_and_refuses_unknowns() {
        assert_eq!(mem_available("MemAvailable: 42 kB\n"), Some(42 * 1024));
        assert_eq!(mem_available("MemAvailable: 42 MB"), None);
        assert!(cgroup_location("1:memory:/group", "").is_none());
        assert!(cgroup_location("0::/../group", "").is_none());
        let (mount, path) = cgroup_location(
            "0::/group/child",
            "1 0 0:1 /group /sys/fs/cgroup rw - cgroup2 cgroup2 rw",
        )
        .unwrap();
        assert_eq!(path, mount.join("child"));
        let dir = tempfile::tempdir().unwrap();
        let child = dir.path().join("child");
        std::fs::create_dir(&child).unwrap();
        std::fs::write(dir.path().join("cgroup.procs"), "").unwrap();
        std::fs::write(
            child.join("cgroup.procs"),
            format!("{}\n", std::process::id()),
        )
        .unwrap();
        let inherited = format!(
            "1 0 0:1 /.. {} rw - cgroup2 cgroup2 rw",
            dir.path().display()
        );
        assert_eq!(
            cgroup_location("0::/", &inherited),
            Some((dir.path().to_owned(), child.clone()))
        );
        for (path, high, max, current) in [
            (dir.path(), "max", "1000", "700"),
            (child.as_path(), "200", "500", "150"),
        ] {
            for (name, value) in [
                ("memory.high", high),
                ("memory.max", max),
                ("memory.current", current),
            ] {
                std::fs::write(path.join(name), value).unwrap();
            }
        }
        assert_eq!(cgroup_headroom(800, dir.path(), &child), Some(50));
        std::fs::write(child.join("memory.high"), "max").unwrap();
        assert_eq!(cgroup_headroom(800, dir.path(), &child), Some(300));
        std::fs::write(child.join("memory.current"), "broken").unwrap();
        assert_eq!(cgroup_headroom(800, dir.path(), &child), None);
    }
}
