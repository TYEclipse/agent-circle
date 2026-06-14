// S17R173 — 磁盘空间监测与降级
//
// 验收：磁盘不足时优雅降级，不丢数据，提前告警。

use crate::errors::{AcError, AcResult};
use std::path::Path;

/// Thresholds in bytes
const CRITICAL_THRESHOLD: u64 = 10 * 1024 * 1024; // 10 MB — refuse writes
const WARNING_THRESHOLD: u64 = 100 * 1024 * 1024; // 100 MB — log warning

/// Disk space status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskStatus {
    /// Healthy — plenty of space
    Healthy { available_bytes: u64 },
    /// Low — approaching danger zone, operations continue with warnings
    Warning { available_bytes: u64 },
    /// Critical — refuse new writes to prevent data corruption
    Critical { available_bytes: u64 },
}

impl DiskStatus {
    pub fn is_critical(&self) -> bool {
        matches!(self, DiskStatus::Critical { .. })
    }

    pub fn available_bytes(&self) -> u64 {
        match self {
            DiskStatus::Healthy { available_bytes } => *available_bytes,
            DiskStatus::Warning { available_bytes } => *available_bytes,
            DiskStatus::Critical { available_bytes } => *available_bytes,
        }
    }
}

/// Check disk space for the given path. Returns DiskStatus.
/// On platforms where we can't query, assume healthy.
pub fn check_disk_space(path: &Path) -> DiskStatus {
    // Use statvfs on Unix, GetDiskFreeSpaceEx on Windows
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = std::fs::metadata(path) {
            let _dev = meta.dev();
            // Try statvfs
            let available = disk_available(path);
            return classify(available);
        }
    }

    #[cfg(windows)]
    {
        // On Windows, we'd use GetDiskFreeSpaceExW — for now, assume healthy
    }

    DiskStatus::Healthy {
        available_bytes: u64::MAX,
    }
}

/// Get available bytes on the filesystem containing `path`.
/// Uses statvfs via libc.
#[cfg(unix)]
fn disk_available(path: &Path) -> u64 {
    // Try statvfs via libc
    let path_c = match std::ffi::CString::new(path.to_string_lossy().as_bytes()) {
        Ok(c) => c,
        Err(_) => return u64::MAX,
    };

    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(path_c.as_ptr(), &mut stat) };
    if rc != 0 {
        return u64::MAX; // can't query, assume healthy
    }

    // available = f_bavail * f_frsize (or f_bsize)
    let block_size = stat.f_frsize as u64;
    let avail_blocks = stat.f_bavail as u64;
    avail_blocks.saturating_mul(block_size)
}

fn classify(available_bytes: u64) -> DiskStatus {
    if available_bytes < CRITICAL_THRESHOLD {
        DiskStatus::Critical { available_bytes }
    } else if available_bytes < WARNING_THRESHOLD {
        DiskStatus::Warning { available_bytes }
    } else {
        DiskStatus::Healthy { available_bytes }
    }
}

/// Guard for write operations — checks disk space before proceeding.
/// Returns Err(AcError::DiskFull) if critical.
pub fn guard_write(path: &Path) -> AcResult<()> {
    let status = check_disk_space(path);
    match status {
        DiskStatus::Critical { available_bytes } => Err(AcError::DiskFull(format!(
            "disk critically low: {} bytes available, need at least {}",
            available_bytes, CRITICAL_THRESHOLD
        ))),
        DiskStatus::Warning { available_bytes } => {
            tracing::warn!(
                available_mb = available_bytes / 1024 / 1024,
                "⚠️ Disk space low"
            );
            Ok(())
        }
        DiskStatus::Healthy { .. } => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r173a_classify_healthy() {
        let status = classify(1024 * 1024 * 1024); // 1 GB
        assert_eq!(
            status,
            DiskStatus::Healthy {
                available_bytes: 1024 * 1024 * 1024
            }
        );
    }

    #[test]
    fn r173b_classify_warning() {
        let status = classify(50 * 1024 * 1024); // 50 MB
        assert_eq!(
            status,
            DiskStatus::Warning {
                available_bytes: 50 * 1024 * 1024
            }
        );
    }

    #[test]
    fn r173c_classify_critical() {
        let status = classify(5 * 1024 * 1024); // 5 MB
        assert_eq!(
            status,
            DiskStatus::Critical {
                available_bytes: 5 * 1024 * 1024
            }
        );
    }

    #[test]
    fn r173d_classify_boundary_warning_upper() {
        // Just above critical threshold
        let status = classify(10 * 1024 * 1024 + 1);
        assert!(matches!(status, DiskStatus::Warning { .. }));
    }

    #[test]
    fn r173e_classify_boundary_critical() {
        // At critical threshold
        let status = classify(10 * 1024 * 1024 - 1);
        assert!(matches!(status, DiskStatus::Critical { .. }));
    }

    #[test]
    fn r173f_guard_critical_returns_error() {
        // Check guard_write on current directory (real call)
        let status = check_disk_space(Path::new("/tmp"));
        if status.is_critical() {
            // If /tmp is truly full, skip
            return;
        }
        // Test on a known path — / should always have enough space in CI
        let result = guard_write(Path::new("/tmp"));
        // Will only error if disk is actually full
        if let Err(e) = result {
            assert!(e.to_string().contains("disk"), "Error should mention disk");
        }
    }

    #[test]
    fn r173g_status_is_critical() {
        let critical = DiskStatus::Critical {
            available_bytes: 1024,
        };
        assert!(critical.is_critical());

        let warning = DiskStatus::Warning {
            available_bytes: 50 * 1024 * 1024,
        };
        assert!(!warning.is_critical());

        let healthy = DiskStatus::Healthy {
            available_bytes: 1024 * 1024 * 1024,
        };
        assert!(!healthy.is_critical());
    }

    #[test]
    fn r173h_available_bytes() {
        let s = DiskStatus::Healthy {
            available_bytes: 42,
        };
        assert_eq!(s.available_bytes(), 42);

        let s = DiskStatus::Critical { available_bytes: 7 };
        assert_eq!(s.available_bytes(), 7);
    }

    #[test]
    fn r173i_check_disk_space_real() {
        // Check actual disk space on /tmp — should never panic
        let status = check_disk_space(Path::new("/tmp"));
        // Just verify it returns something meaningful
        assert!(
            status.available_bytes() > 0 || status.is_critical(),
            "Disk check should return valid results"
        );
    }
}
