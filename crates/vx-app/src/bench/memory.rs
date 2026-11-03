use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

const BYTES_PER_MIB: f64 = 1_048_576.0;

/// GPU memory from wgpu's allocator (Vulkan and D3D12 only), taken after the timed frames while
/// scene buffers and render targets are alive, grouped by resource label.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GpuMemoryReport {
    pub allocated_mib: f64,
    pub reserved_mib: f64,
    pub allocated_mib_by_label: BTreeMap<String, f64>,
}

pub fn gpu_memory(device: &wgpu::Device) -> Option<GpuMemoryReport> {
    let report = device.generate_allocator_report()?;
    let mut by_label = BTreeMap::new();
    for allocation in &report.allocations {
        let label = if allocation.name.is_empty() {
            "(unlabelled)".to_owned()
        } else {
            allocation.name.clone()
        };
        *by_label.entry(label).or_insert(0.0) += allocation.size as f64 / BYTES_PER_MIB;
    }
    Some(GpuMemoryReport {
        allocated_mib: report.total_allocated_bytes as f64 / BYTES_PER_MIB,
        reserved_mib: report.total_reserved_bytes as f64 / BYTES_PER_MIB,
        allocated_mib_by_label: by_label,
    })
}

#[cfg(target_os = "linux")]
pub fn peak_resident_mib() -> Option<f64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let kib: f64 = status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))?
        .trim()
        .trim_end_matches("kB")
        .trim()
        .parse()
        .ok()?;
    Some(kib / 1024.0)
}

#[cfg(windows)]
#[allow(unsafe_code)]
pub fn peak_resident_mib() -> Option<f64> {
    use windows_sys::Win32::System::{
        ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
        Threading::GetCurrentProcess,
    };
    let size = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    // SAFETY: an all-zero PROCESS_MEMORY_COUNTERS is valid, the pseudo handle from
    // GetCurrentProcess needs no closing, and `size` is the struct's exact byte size.
    let counters = unsafe {
        let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        counters.cb = size;
        (GetProcessMemoryInfo(GetCurrentProcess(), &raw mut counters, size) != 0)
            .then_some(counters)
    }?;
    Some(counters.PeakWorkingSetSize as f64 / BYTES_PER_MIB)
}

#[cfg(not(any(target_os = "linux", windows)))]
pub fn peak_resident_mib() -> Option<f64> {
    None
}

#[cfg(test)]
mod tests {
    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn peak_resident_is_reported_and_positive() {
        let peak = super::peak_resident_mib().expect("peak RSS is readable on this OS");
        assert!(peak > 0.0);
    }
}
