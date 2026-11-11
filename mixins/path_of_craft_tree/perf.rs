// Time spent on the mod's hot paths (docs/perf-spec.md), written to the log twice a minute.
// Only a build with PCT_PERF set at compile time measures; any other build compiles every probe
// down to nothing, so players never carry it.
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub const ON: bool = option_env!("PCT_PERF").is_some();
const REPORT_EVERY: Duration = Duration::from_secs(30);

#[derive(Clone, Copy)]
pub enum Probe {
    PlayerTick,
    ServerTick,
    BreakSpeed,
    Grant,
    Destroy,
    Loot,
    TreeDraw,
    ClientFrame,
    OreScan,
    StateBytes,
    LookNew,
    AtlasCompose,
    AtlasUpload,
}

const PROBES: usize = 13;
const NAMES: [&str; PROBES] = [
    "player_tick",
    "server_tick",
    "break_speed",
    "grant",
    "destroy",
    "loot",
    "tree_draw",
    "client_frame",
    "ore_scan",
    "state_bytes",
    "look_new",
    "atlas_compose",
    "atlas_upload",
];

static CALLS: [AtomicU64; PROBES] = [const { AtomicU64::new(0) }; PROBES];
static TOTAL: [AtomicU64; PROBES] = [const { AtomicU64::new(0) }; PROBES];
static PEAK: [AtomicU64; PROBES] = [const { AtomicU64::new(0) }; PROBES];
static SINCE: Mutex<Option<Instant>> = Mutex::new(None);

/// Measures until dropped: `let _timer = perf::time(Probe::Grant);`.
pub struct Timer(Option<(Probe, Instant)>);

pub fn time(probe: Probe) -> Timer {
    Timer(ON.then(|| (probe, Instant::now())))
}

impl Drop for Timer {
    fn drop(&mut self) {
        if let Some((probe, started)) = self.0 {
            add(probe, started.elapsed().as_nanos() as u64);
        }
    }
}

/// Counts something that is not a time, such as the bytes of a message.
pub fn add(probe: Probe, amount: u64) {
    if !ON {
        return;
    }
    let i = probe as usize;
    CALLS[i].fetch_add(1, Ordering::Relaxed);
    TOTAL[i].fetch_add(amount, Ordering::Relaxed);
    PEAK[i].fetch_max(amount, Ordering::Relaxed);
}

/// Writes and clears the counters once `REPORT_EVERY` has passed; call it every tick or frame.
pub fn report_if_due(side: &str) {
    if !ON {
        return;
    }
    let seconds = {
        let mut since = SINCE.lock().unwrap_or_else(|e| e.into_inner());
        let start = *since.get_or_insert_with(Instant::now);
        if start.elapsed() < REPORT_EVERY {
            return;
        }
        *since = Some(Instant::now());
        start
            .elapsed()
            .as_secs_f64()
            .max(REPORT_EVERY.as_secs_f64())
    };
    for (i, name) in NAMES.iter().enumerate() {
        let calls = CALLS[i].swap(0, Ordering::Relaxed);
        let total = TOTAL[i].swap(0, Ordering::Relaxed);
        let peak = PEAK[i].swap(0, Ordering::Relaxed);
        if calls == 0 {
            continue;
        }
        let mean = total as f64 / calls as f64;
        if i == Probe::StateBytes as usize {
            log::info!(
                "pct perf {side} {name}: messages {calls} ({:.2}/s), mean {mean:.0} B, peak {peak} B",
                calls as f64 / seconds,
            );
            continue;
        }
        log::info!(
            "pct perf {side} {name}: calls {calls} ({:.1}/s), mean {:.2} us, peak {:.2} us, total {:.3} ms/s",
            calls as f64 / seconds,
            mean / 1000.0,
            peak as f64 / 1000.0,
            total as f64 / 1.0e6 / seconds,
        );
    }
}
