use super::crossdev;
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

const BYTES_PER_KB: f64 = 1000.0;
const BYTES_PER_KIB: f64 = 1024.0;
const BYTES_PER_MB: f64 = 1_000_000.0;
const BYTES_PER_MIB: f64 = 1_048_576.0;
const BYTES_PER_GB: f64 = 1_000_000_000.0;
const BYTES_PER_GIB: f64 = 1_073_741_824.0;
const BYTES_PER_TB: f64 = 1_000_000_000_000.0;
const BYTES_PER_TIB: f64 = 1_099_511_627_776.0;

#[derive(Debug, Clone, Copy)]
pub enum ByteFormat {
    Metric,
    Binary,
    Bytes,
    GB,
    GiB,
    MB,
    MiB,
}

impl ByteFormat {
    pub fn display(self, bytes: u128) -> impl fmt::Display {
        ByteFormatDisplay {
            format: self,
            bytes,
        }
    }
}

struct ByteFormatDisplay {
    format: ByteFormat,
    bytes: u128,
}

impl fmt::Display for ByteFormatDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        use ByteFormat::*;
        let bytes = self.bytes as f64;

        match self.format {
            Bytes => write!(f, "{} B", self.bytes),
            GB => write!(f, "{:.2} GB", bytes / BYTES_PER_GB),
            GiB => write!(f, "{:.2} GiB", bytes / BYTES_PER_GIB),
            MB => write!(f, "{:.2} MB", bytes / BYTES_PER_MB),
            MiB => write!(f, "{:.2} MiB", bytes / BYTES_PER_MIB),
            Metric => format_auto(f, bytes, false),
            Binary => format_auto(f, bytes, true),
        }
    }
}

fn format_auto(f: &mut fmt::Formatter<'_>, bytes: f64, binary: bool) -> fmt::Result {
    let (divisors, units): (&[f64], &[&str]) = if binary {
        (
            &[BYTES_PER_TIB, BYTES_PER_GIB, BYTES_PER_MIB, BYTES_PER_KIB, 1.0],
            &["TiB", "GiB", "MiB", "KiB", "B"],
        )
    } else {
        (
            &[BYTES_PER_TB, BYTES_PER_GB, BYTES_PER_MB, BYTES_PER_KB, 1.0],
            &["TB", "GB", "MB", "KB", "B"],
        )
    };

    for (divisor, unit) in divisors.iter().zip(units.iter()) {
        if bytes >= *divisor {
            return write!(f, "{:.2} {}", bytes / divisor, unit);
        }
    }
    write!(f, "{:.0} B", bytes)
}

#[derive(Debug, Clone)]
pub enum TraversalSorting {
    None,
    AlphabeticalByFileName,
}

#[derive(Debug)]
pub struct Throttle {
    trigger: Arc<AtomicBool>,
}

impl Throttle {
    pub fn new(duration: Duration, initial_sleep: Option<Duration>) -> Self {
        let instance = Self {
            trigger: Default::default(),
        };

        let trigger = Arc::downgrade(&instance.trigger);
        std::thread::spawn(move || {
            if let Some(duration) = initial_sleep {
                std::thread::sleep(duration);
            }
            while let Some(t) = trigger.upgrade() {
                t.store(true, Ordering::Relaxed);
                std::thread::sleep(duration);
            }
        });

        instance
    }

    pub fn can_update(&self) -> bool {
        self.trigger.swap(false, Ordering::Relaxed)
    }
}

#[derive(Debug, Clone)]
pub struct WalkOptions {
    pub threads: usize,
    pub count_hard_links: bool,
    pub apparent_size: bool,
    pub sorting: TraversalSorting,
    pub cross_filesystems: bool,
    pub ignore_dirs: BTreeSet<PathBuf>,
}

impl Default for WalkOptions {
    fn default() -> Self {
        Self {
            threads: num_cpus::get(),
            count_hard_links: false,
            apparent_size: false,
            sorting: TraversalSorting::None,
            cross_filesystems: false,
            ignore_dirs: BTreeSet::new(),
        }
    }
}

type WalkDir = jwalk::WalkDirGeneric<((), Option<Result<std::fs::Metadata, jwalk::Error>>)>;

impl WalkOptions {
    pub fn iter_from_path(&self, root: &Path, root_device_id: u64, skip_root: bool) -> WalkDir {
        let ignore_dirs = self.ignore_dirs.clone();
        let cwd = std::env::current_dir().unwrap_or_else(|_| root.to_owned());
        let cross_filesystems = self.cross_filesystems;

        WalkDir::new(root)
            .follow_links(false)
            .min_depth(if skip_root { 1 } else { 0 })
            .sort(match self.sorting {
                TraversalSorting::None => false,
                TraversalSorting::AlphabeticalByFileName => true,
            })
            .skip_hidden(false)
            .process_read_dir(move |_, _, _, dir_entry_results| {
                dir_entry_results.iter_mut().for_each(|dir_entry_result| {
                    if let Ok(dir_entry) = dir_entry_result {
                        let metadata = dir_entry.metadata();

                        if dir_entry.file_type.is_dir() {
                            let ok_for_fs = cross_filesystems
                                || metadata
                                    .as_ref()
                                    .map(|m| crossdev::is_same_device(root_device_id, m))
                                    .unwrap_or(true);
                            if !ok_for_fs
                                || ignore_directory(&dir_entry.path(), &ignore_dirs, &cwd)
                            {
                                dir_entry.read_children_path = None;
                            }
                        }

                        dir_entry.client_state = Some(metadata);
                    }
                });
            })
            .parallelism(match self.threads {
                0 => jwalk::Parallelism::RayonDefaultPool {
                    busy_timeout: std::time::Duration::from_secs(1),
                },
                1 => jwalk::Parallelism::Serial,
                _ => jwalk::Parallelism::RayonExistingPool {
                    pool: jwalk::rayon::ThreadPoolBuilder::new()
                        .stack_size(128 * 1024)
                        .num_threads(self.threads)
                        .thread_name(|idx| format!("rustdirstat-walk-{idx}"))
                        .build()
                        .expect("thread pool build should not fail")
                        .into(),
                    busy_timeout: None,
                },
            })
    }
}

#[derive(Debug, Default)]
pub struct WalkResult {
    pub num_errors: u64,
}

impl WalkResult {
    pub fn to_exit_code(&self) -> i32 {
        i32::from(self.num_errors > 0)
    }
}

fn ignore_directory(path: &Path, ignore_dirs: &BTreeSet<PathBuf>, _cwd: &Path) -> bool {
    if ignore_dirs.is_empty() {
        return false;
    }
    match std::fs::canonicalize(path) {
        Ok(canonical) => ignore_dirs.contains(&canonical),
        Err(_) => false,
    }
}
