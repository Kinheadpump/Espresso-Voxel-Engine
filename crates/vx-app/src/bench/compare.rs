use std::path::Path;

use anyhow::{Context, Result, bail};

use super::{
    report::{BenchReport, SCHEMA_VERSION},
    stats::Metric,
    visibility::VisibilityCounters,
};

const BYTES_PER_MIB: f64 = 1_048_576.0;
const MIN_NOISE_FRACTION: f64 = 0.02;
const MIN_REPEATS_FOR_VERDICT: usize = 3;

fn load(path: &Path) -> Result<BenchReport> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

fn timing_metrics(report: &BenchReport) -> Vec<(&'static str, Option<&Metric>)> {
    let results = &report.results;
    vec![
        ("world generation", Some(&results.world_generation_ms)),
        ("meshing", Some(&results.meshing_ms)),
        ("upload", Some(&results.upload_ms)),
        ("frame CPU encode", Some(&results.frame_cpu_encode_us)),
        (
            "frame GPU terrain pass",
            results.frame_gpu_terrain_pass_us.as_ref(),
        ),
        ("frame wall (CPU+GPU, synced)", Some(&results.frame_wall_us)),
    ]
}

pub fn print_report(report: &BenchReport) {
    println!(
        "scenario {} | commit {}{} | {} | {} ({} cores) | {} {} {}",
        report.scenario.name,
        report.build.git_commit,
        if report.build.git_dirty == "false" {
            ""
        } else {
            " (dirty)"
        },
        report.build.profile,
        report.system.cpu,
        report
            .system
            .physical_cores
            .map_or("?".into(), |cores| cores.to_string()),
        report.gpu.name,
        report.gpu.backend,
        report.gpu.driver_info,
    );
    println!(
        "content: {} chunks generated, {} meshed, {} quads, {} draws, hash {} | voxel heap {:.1} MiB | geometry buffers {:.1} MiB | RSS {}",
        report.content.chunks_generated,
        report.content.chunks_meshed,
        report.content.quads,
        report.content.draws,
        report.content.mesh_hash,
        report.content.voxel_heap_bytes as f64 / BYTES_PER_MIB,
        report.content.geometry_buffer_bytes as f64 / BYTES_PER_MIB,
        report
            .results
            .process_resident_mib
            .map_or("n/a".into(), |mib| format!("{mib:.0} MiB")),
    );
    println!(
        "{:<30} {:>10} {:>10} {:>10} {:>10} {:>8}",
        "metric", "center", "p50", "p99", "max", "spread"
    );
    for (name, metric) in timing_metrics(report) {
        match metric {
            Some(metric) => println!(
                "{:<30} {:>10.1} {:>10.1} {:>10.1} {:>10.1} {:>7.1}%  [{}]",
                name,
                metric.center(),
                metric.pooled.p50,
                metric.pooled.p99,
                metric.pooled.max,
                metric.relative_spread() * 100.0,
                metric.unit,
            ),
            None => println!("{name:<30} {:>10}", "n/a"),
        }
    }
    if let Some(visibility) = &report.visibility {
        print_culling_ladder(visibility);
    }
}

fn print_culling_ladder(visibility: &VisibilityCounters) {
    let Some(resident) = visibility.ladder.first() else {
        return;
    };
    println!(
        "\nculling ladder (mean of {} path frames, if culling stopped at each stage)",
        visibility.sampled_frames
    );
    println!(
        "{:<16} {:>10} {:>10} {:>12} {:>8}",
        "stage", "chunks", "draws", "quads", "quads %"
    );
    for rung in &visibility.ladder {
        println!(
            "{:<16} {:>10.1} {:>10.1} {:>12.0} {:>7.1}%",
            rung.stage,
            rung.chunks,
            rung.draws,
            rung.quads,
            100.0 * rung.quads / resident.quads.max(1.0)
        );
    }
}

fn comparability_warnings(baseline: &BenchReport, candidate: &BenchReport) -> Vec<String> {
    let mut warnings = Vec::new();
    if baseline.system != candidate.system || baseline.gpu != candidate.gpu {
        warnings
            .push("different machine or GPU/driver: absolute timings are not comparable".into());
    }
    if baseline.content != candidate.content {
        warnings.push(format!(
            "rendered content differs (quads {} -> {}, hash {} -> {}): timings cover different work",
            baseline.content.quads, candidate.content.quads, baseline.content.mesh_hash, candidate.content.mesh_hash
        ));
    }
    if baseline.build.rustc != candidate.build.rustc
        || baseline.build.rustflags != candidate.build.rustflags
    {
        warnings.push("different compiler or RUSTFLAGS".into());
    }
    for (label, report) in [("baseline", baseline), ("candidate", candidate)] {
        if report.build.git_dirty != "false" {
            warnings.push(format!(
                "{label} was built from a dirty tree ({})",
                report.build.git_commit
            ));
        }
        if report.gpu.is_software() {
            warnings.push(format!(
                "{label} ran on a software adapter: GPU numbers are meaningless"
            ));
        }
    }
    warnings
}

fn verdict(baseline: &Metric, candidate: &Metric) -> (f64, f64, &'static str) {
    let change = (candidate.center() - baseline.center()) / baseline.center();
    let noise = baseline
        .relative_spread()
        .max(candidate.relative_spread())
        .max(MIN_NOISE_FRACTION);
    let repeats = baseline
        .per_repeat_p50
        .len()
        .min(candidate.per_repeat_p50.len());
    let label = if repeats < MIN_REPEATS_FOR_VERDICT {
        "too few repeats"
    } else if change.abs() <= noise {
        "within noise"
    } else if change < 0.0 {
        "FASTER"
    } else {
        "SLOWER"
    };
    (change, noise, label)
}

pub fn compare(baseline_path: &Path, candidate_path: &Path) -> Result<()> {
    let baseline = load(baseline_path)?;
    let candidate = load(candidate_path)?;
    if baseline.schema != SCHEMA_VERSION || candidate.schema != SCHEMA_VERSION {
        bail!(
            "schema mismatch: expected {SCHEMA_VERSION}, got {} and {}",
            baseline.schema,
            candidate.schema
        );
    }
    if baseline.build.debug_assertions || candidate.build.debug_assertions {
        bail!("refusing to compare debug builds");
    }
    if baseline.scenario != candidate.scenario {
        bail!(
            "scenario parameters differ:\n  baseline  {:?}\n  candidate {:?}",
            baseline.scenario,
            candidate.scenario
        );
    }
    println!(
        "baseline : {} ({})",
        baseline_path.display(),
        baseline.build.git_commit
    );
    println!(
        "candidate: {} ({})",
        candidate_path.display(),
        candidate.build.git_commit
    );
    for warning in comparability_warnings(&baseline, &candidate) {
        println!("WARNING: {warning}");
    }
    println!(
        "\n{:<30} {:>11} {:>11} {:>8} {:>7}  verdict",
        "metric (median of repeat medians)", "baseline", "candidate", "change", "noise"
    );
    for ((name, base), (_, cand)) in timing_metrics(&baseline)
        .into_iter()
        .zip(timing_metrics(&candidate))
    {
        let (Some(base), Some(cand)) = (base, cand) else {
            println!("{name:<30} {:>11}", "n/a");
            continue;
        };
        let (change, noise, label) = verdict(base, cand);
        println!(
            "{:<30} {:>9.1}{:<2} {:>9.1}{:<2} {:>+7.1}% {:>6.1}%  {}",
            name,
            base.center(),
            base.unit,
            cand.center(),
            cand.unit,
            change * 100.0,
            noise * 100.0,
            label
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metric(repeat_medians: &[f64]) -> Metric {
        Metric::from_single_samples("ms", repeat_medians)
    }

    #[test]
    fn change_inside_repeat_spread_is_noise() {
        let (_, _, label) = verdict(
            &metric(&[100.0, 104.0, 98.0]),
            &metric(&[101.0, 103.0, 99.0]),
        );
        assert_eq!(label, "within noise");
    }

    #[test]
    fn clear_improvement_is_reported_faster() {
        let (change, _, label) =
            verdict(&metric(&[100.0, 101.0, 99.0]), &metric(&[80.0, 81.0, 79.0]));
        assert_eq!(label, "FASTER");
        assert!((change + 0.2).abs() < 1e-9);
    }

    #[test]
    fn single_repeat_gives_no_verdict() {
        let (_, _, label) = verdict(&metric(&[100.0]), &metric(&[50.0]));
        assert_eq!(label, "too few repeats");
    }
}
