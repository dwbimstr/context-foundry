//! Native controls applied before model loading. No cgroup/VM guarantee is
//! inferred from running in a subprocess. Missing native enforcement refuses.
use super::Config;
use std::io;

/// Shared descriptor-only Linux enforcement for owned generation and embedding.
#[cfg(target_os = "linux")]
pub fn install_cpu(
    threads: u32,
    memory_bytes: u64,
    cpu_seconds: u64,
    model_fd: i32,
) -> io::Result<()> {
    if !(1..=64).contains(&threads)
        || !(128 * 1024 * 1024..=128 * 1024 * 1024 * 1024).contains(&memory_bytes)
        || !(1..=86400).contains(&cpu_seconds)
    {
        return Err(io::Error::other("invalid CPU resource limits"));
    }
    linux::install(threads, memory_bytes, cpu_seconds, model_fd)
}

pub fn install(config: &Config, model_fd: i32) -> io::Result<&'static str> {
    config.platform().map_err(io::Error::other)?;
    #[cfg(target_os = "linux")]
    {
        install_cpu(
            config.threads,
            config.memory_bytes,
            config.cpu_seconds,
            model_fd,
        )?;
        Ok("linux-seccomp-fd-v1")
    }
    #[cfg(target_os = "macos")]
    {
        let _ = model_fd;
        macos::install(config)?;
        Ok("macos-app-sandbox-development-v1")
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = model_fd;
        Err(io::Error::other("native isolation unavailable"))
    }
}

pub fn memory(pid: u32) -> io::Result<u64> {
    #[cfg(target_os = "linux")]
    {
        let fields = std::fs::read_to_string(format!("/proc/{pid}/statm"))?;
        let pages: u64 = fields
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| io::Error::other("missing RSS"))?
            .parse()
            .map_err(io::Error::other)?;
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if page <= 0 {
            return Err(io::Error::other("invalid page size"));
        }
        pages
            .checked_mul(page as u64)
            .ok_or_else(|| io::Error::other("RSS overflow"))
    }
    #[cfg(target_os = "macos")]
    {
        crate::neural::supervisor::physical_footprint(pid, "knowledge").map_err(io::Error::other)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
        Err(io::Error::other("memory measurement unavailable"))
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    fn check(rc: libc::c_long) -> io::Result<()> {
        if rc < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    pub fn install(
        threads: u32,
        memory_bytes: u64,
        cpu_seconds: u64,
        model_fd: i32,
    ) -> io::Result<()> {
        // Inherited hard affinity applies to every later model/monitor thread.
        let mut available = unsafe { std::mem::zeroed::<libc::cpu_set_t>() };
        check(
            unsafe {
                libc::sched_getaffinity(0, std::mem::size_of_val(&available), &mut available)
            }
            .into(),
        )?;
        let mut allowed = unsafe { std::mem::zeroed::<libc::cpu_set_t>() };
        let mut count = 0;
        for cpu in 0..libc::CPU_SETSIZE as usize {
            if unsafe { libc::CPU_ISSET(cpu, &available) } && count < threads {
                unsafe { libc::CPU_SET(cpu, &mut allowed) };
                count += 1;
            }
        }
        if count == 0 {
            return Err(io::Error::other("no available CPU"));
        }
        check(
            unsafe { libc::sched_setaffinity(0, std::mem::size_of_val(&allowed), &allowed) }.into(),
        )?;
        check(unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, 10) }.into())?;
        for (resource, value) in [
            (libc::RLIMIT_AS, memory_bytes),
            (libc::RLIMIT_CPU, cpu_seconds),
            (libc::RLIMIT_CORE, 0),
            (libc::RLIMIT_FSIZE, 0),
            (libc::RLIMIT_NOFILE, 64),
        ] {
            let limit = libc::rlimit {
                rlim_cur: value,
                rlim_max: value,
            };
            check(unsafe { libc::setrlimit(resource, &limit) }.into())?;
        }
        check(unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) }.into())?;
        let flags = unsafe { libc::fcntl(model_fd, libc::F_GETFL) };
        if flags < 0 || flags & libc::O_ACCMODE != libc::O_RDONLY {
            return Err(io::Error::other("model descriptor must be read-only"));
        }
        seccomp()
    }
    fn seccomp() -> io::Result<()> {
        use libc::{sock_filter as Filter, sock_fprog as Program};
        const LD: u16 = 0x20;
        const EQ: u16 = 0x15;
        const SET: u16 = 0x45;
        const RET: u16 = 0x06;
        const ALLOW: u32 = 0x7fff0000;
        const ERRNO: u32 = 0x00050000;
        const KILL: u32 = 0x80000000;
        #[cfg(target_arch = "x86_64")]
        const ARCH: u32 = 0xc000003e;
        #[cfg(target_arch = "aarch64")]
        const ARCH: u32 = 0xc00000b7;
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        compile_error!("local worker seccomp needs a reviewed audit architecture");
        let ins = |code, jt, jf, k| Filter { code, jt, jf, k };
        let mut code = vec![
            ins(LD, 0, 0, 4),
            ins(EQ, 1, 0, ARCH),
            ins(RET, 0, 0, KILL),
            ins(LD, 0, 0, 0),
        ];
        #[cfg(target_arch = "x86_64")]
        code.extend([ins(SET, 0, 1, 0x40000000), ins(RET, 0, 0, KILL)]); // x32 ABI
        // clone3's pointer arguments cannot be inspected by classic BPF. libc
        // falls back to clone; permit only same-process thread creation there.
        code.extend([
            ins(EQ, 0, 1, libc::SYS_clone3 as u32),
            ins(RET, 0, 0, ERRNO | libc::ENOSYS as u32),
        ]);
        code.extend([
            ins(EQ, 0, 4, libc::SYS_clone as u32),
            ins(LD, 0, 0, 16),
            ins(SET, 1, 0, libc::CLONE_THREAD as u32),
            ins(RET, 0, 0, ERRNO | libc::EPERM as u32),
            ins(RET, 0, 0, ALLOW),
        ]);
        // prlimit64 may inspect limits, never change them (arg 2 new_limit).
        code.extend([
            ins(EQ, 0, 6, libc::SYS_prlimit64 as u32),
            ins(LD, 0, 0, 32),
            ins(EQ, 0, 3, 0),
            ins(LD, 0, 0, 36),
            ins(EQ, 0, 1, 0),
            ins(RET, 0, 0, ALLOW),
            ins(RET, 0, 0, ERRNO | libc::EPERM as u32),
        ]);
        // Rust/glibc may signal their own threads, but never another process.
        code.extend([
            ins(EQ, 0, 4, libc::SYS_tgkill as u32),
            ins(LD, 0, 0, 16),
            ins(EQ, 1, 0, std::process::id()),
            ins(RET, 0, 0, ERRNO | libc::EPERM as u32),
            ins(RET, 0, 0, ALLOW),
        ]);
        // Do not admit F_SETOWN/F_SETSIG: async descriptor notification can
        // signal another process even when kill/tgkill are restricted.
        let fcntl = code.len();
        code.push(ins(EQ, 0, 0, libc::SYS_fcntl as u32));
        code.push(ins(LD, 0, 0, 24)); // arg 1: command
        for command in [
            libc::F_GETFD,
            libc::F_SETFD,
            libc::F_GETFL,
            libc::F_DUPFD,
            libc::F_DUPFD_CLOEXEC,
        ] {
            code.extend([ins(EQ, 0, 1, command as u32), ins(RET, 0, 0, ALLOW)]);
        }
        code.extend([
            ins(EQ, 0, 3, libc::F_SETFL as u32),
            ins(LD, 0, 0, 32), // arg 2: flags
            ins(SET, 1, 0, libc::O_ASYNC as u32),
            ins(RET, 0, 0, ALLOW),
            ins(RET, 0, 0, ERRNO | libc::EPERM as u32),
        ]);
        code[fcntl].jf = (code.len() - fcntl - 1) as u8;
        // A descriptor-only profile: no pathname opens, filesystem mutation,
        // network, exec, process inspection, io_uring or future syscalls. The
        // pinned backend loads its GGUF through FILE*, not a /dev/fd reopen.
        let allowed = [
            libc::SYS_read,
            libc::SYS_write,
            libc::SYS_close,
            libc::SYS_fstat,
            libc::SYS_lseek,
            libc::SYS_pread64,
            libc::SYS_readv,
            libc::SYS_writev,
            libc::SYS_mmap,
            libc::SYS_mprotect,
            libc::SYS_munmap,
            libc::SYS_mremap,
            libc::SYS_brk,
            libc::SYS_madvise,
            libc::SYS_rt_sigaction,
            libc::SYS_rt_sigprocmask,
            libc::SYS_rt_sigreturn,
            libc::SYS_sigaltstack,
            libc::SYS_getpid,
            libc::SYS_getppid,
            libc::SYS_gettid,
            libc::SYS_getuid,
            libc::SYS_geteuid,
            libc::SYS_getgid,
            libc::SYS_getegid,
            libc::SYS_getgroups,
            libc::SYS_uname,
            libc::SYS_sysinfo,
            libc::SYS_clock_gettime,
            libc::SYS_clock_nanosleep,
            libc::SYS_nanosleep,
            libc::SYS_gettimeofday,
            libc::SYS_getrusage,
            libc::SYS_sched_getaffinity,
            libc::SYS_sched_yield,
            libc::SYS_getcpu,
            libc::SYS_futex,
            libc::SYS_set_robust_list,
            libc::SYS_get_robust_list,
            libc::SYS_set_tid_address,
            libc::SYS_rseq,
            libc::SYS_membarrier,
            libc::SYS_getrandom,
            libc::SYS_exit,
            libc::SYS_exit_group,
            libc::SYS_dup,
            libc::SYS_dup3,
            libc::SYS_ppoll,
            libc::SYS_getrlimit,
        ];
        #[cfg(target_arch = "x86_64")]
        let extra = [
            libc::SYS_arch_prctl,
            libc::SYS_time,
            libc::SYS_poll,
            libc::SYS_dup2,
        ];
        #[cfg(not(target_arch = "x86_64"))]
        let extra: [libc::c_long; 0] = [];
        for syscall in allowed.into_iter().chain(extra) {
            code.extend([ins(EQ, 0, 1, syscall as u32), ins(RET, 0, 0, ALLOW)]);
        }
        code.push(ins(RET, 0, 0, ERRNO | libc::EPERM as u32));
        let program = Program {
            len: code.len() as u16,
            filter: code.as_mut_ptr(),
        };
        check(unsafe { libc::syscall(libc::SYS_seccomp, 1, 0, &program) })
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    unsafe extern "C" {
        fn sandbox_check(pid: libc::pid_t, operation: *const libc::c_char, filter: i32, ...)
        -> i32;
    }
    pub fn install(config: &Config) -> io::Result<()> {
        // The executable must already carry the development App Sandbox
        // signature; this check does not install a deprecated ad-hoc profile.
        if unsafe { libc::geteuid() } == 0 {
            return Err(io::Error::other(
                "non-root worker required for process limits",
            ));
        }
        let denied = unsafe { sandbox_check(libc::getpid(), c"network-outbound".as_ptr(), 0) };
        if denied <= 0 {
            return Err(io::Error::other("App Sandbox network denial required"));
        }
        for (resource, value) in [
            (libc::RLIMIT_CPU, config.cpu_seconds),
            (libc::RLIMIT_NPROC, 0),
            (libc::RLIMIT_CORE, 0),
        ] {
            let limit = libc::rlimit {
                rlim_cur: value,
                rlim_max: value,
            };
            if unsafe { libc::setrlimit(resource, &limit) } != 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }
}
