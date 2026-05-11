/// Lightweight strace facility for StarryOS.
///
/// When the `strace` cargo feature is enabled, syscalls are recorded in a
/// fixed-size buffer and exposed via `/proc/strace`.
///
/// ## Filters (write to `/proc/strace` to configure)
///
/// Format: `<proc_filter>[:<syscall_filter>]`
///
///   `jcode`               – record all jcode syscalls
///   `jcode:connect,socket,sendto,recvfrom,epoll_pwait,epoll_ctl`
///                         – record only network/epoll calls from jcode
///   `:connect,socket`     – record connect+socket from ALL processes
///
/// Empty filter = record everything.
///
/// ## Buffer behaviour
///
/// The buffer stops accepting new entries once full (stop-when-full).
/// This preserves the EARLIEST syscalls — which are the interesting ones
/// (connect, sendto, etc.) — rather than the most recent noise.
///
/// Read `/proc/strace` to drain and clear the buffer.
/// Write `--clear` to `/proc/strace` to clear without reading.
#[cfg(feature = "strace")]
pub mod imp {
    use alloc::{
        format,
        string::{String, ToString},
        vec::Vec,
    };

    use ax_kspin::SpinNoIrq;
    use lazy_static::lazy_static;

    /// Max buffer size. Stop accepting new entries when full.
    const MAX_BUF: usize = 2 * 1024 * 1024; // 2 MiB

    struct State {
        buf: Vec<u8>,
        /// Filter on process name (empty = match all).
        proc_filter: String,
        /// Comma-separated syscall names to record (empty = record all).
        syscall_filter: Vec<String>,
        /// When true, new entries are silently dropped (buffer full).
        full: bool,
    }

    lazy_static! {
        static ref STATE: SpinNoIrq<State> = SpinNoIrq::new(State {
            buf: Vec::new(),
            proc_filter: String::new(),
            syscall_filter: Vec::new(),
            full: false,
        });
    }

    /// Record one syscall. Called from `handle_syscall` after dispatch.
    pub fn record(task_name: &str, tid: u64, sysno: &str, args: [usize; 6], ret: isize) {
        let mut st = STATE.lock();

        if st.full {
            return;
        }

        // Process name filter.
        if !st.proc_filter.is_empty() && !task_name.contains(st.proc_filter.as_str()) {
            return;
        }

        // Syscall name filter (case-insensitive prefix match).
        if !st.syscall_filter.is_empty() {
            let sysno_lower = sysno.to_lowercase();
            let pass = st
                .syscall_filter
                .iter()
                .any(|f| sysno_lower.contains(f.as_str()));
            if !pass {
                return;
            }
        }

        let line = format!(
            "{task_name}[{tid}] {sysno}({:#x},{:#x},{:#x},{:#x},{:#x},{:#x}) = {ret}\n",
            args[0], args[1], args[2], args[3], args[4], args[5],
        );
        let bytes = line.as_bytes();

        if st.buf.len() + bytes.len() > MAX_BUF {
            // Mark full and append a sentinel so the reader knows.
            let sentinel = b"[strace: buffer full, oldest entries preserved]\n";
            if st.buf.len() + sentinel.len() <= MAX_BUF {
                st.buf.extend_from_slice(sentinel);
            }
            st.full = true;
            return;
        }
        st.buf.extend_from_slice(bytes);
    }

    /// Drain the buffer and reset (used by `/proc/strace` read).
    pub fn drain() -> Vec<u8> {
        let mut st = STATE.lock();
        let out = st.buf.clone();
        st.buf.clear();
        st.full = false;
        out
    }

    /// Parse and apply a filter string written to `/proc/strace`.
    ///
    /// Format: `<proc>[:<syscall1>,<syscall2>,...]`
    /// Special value `--clear` clears the buffer without reading.
    pub fn set_filter(raw: &str) {
        let raw = raw.trim_end_matches('\n').trim();
        let mut st = STATE.lock();

        if raw == "--clear" {
            st.buf.clear();
            st.full = false;
            return;
        }

        let (proc_part, syscall_part) = match raw.find(':') {
            Some(pos) => (&raw[..pos], &raw[pos + 1..]),
            None => (raw, ""),
        };

        st.proc_filter = proc_part.to_string();
        st.syscall_filter = if syscall_part.is_empty() {
            Vec::new()
        } else {
            syscall_part
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect()
        };

        // Clear buffer and reset full flag when filter changes.
        st.buf.clear();
        st.full = false;
    }
}

#[cfg(not(feature = "strace"))]
pub mod imp {}
