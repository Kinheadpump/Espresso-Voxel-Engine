use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use vx_render::Shading;

use crate::bench::{BenchOptions, Scenario};

pub enum Mode {
    Windowed,
    Probe,
    Screenshot(PathBuf),
    Bench(BenchOptions),
    Compare {
        baseline: PathBuf,
        candidate: PathBuf,
        allow_shading_change: bool,
    },
}

pub struct Options {
    pub mode: Mode,
    pub view_radius: u32,
    pub vsync: bool,
    pub shading: Shading,
    pub screenshot_view: Option<ScenarioView>,
}

#[derive(Clone, Copy)]
pub struct ScenarioView {
    pub scenario: Scenario,
    pub progress: f32,
}

const USAGE: &str = "usage:
  vx [--radius <chunks>] [--vsync] [--shading flat|textured]   windowed fly-cam
  vx --probe                                       print adapter, tier, features
  vx --screenshot <file.png> [--radius <chunks>] [--shading flat|textured]
     [--scenario <name> [--at 0..1]]               ... from a bench scenario's camera path
  vx bench [--scenario small|default|far|cave|mountain|fly-fast] [--repeats N] [--frames N] [--warmup N]
           [--threads N] [--resolution WxH] [--shading flat|textured] [--out <file.json>]
  vx bench-compare <baseline.json> <candidate.json> [--allow-shading-change]";

fn next_value<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<T> {
    let raw = args
        .next()
        .with_context(|| format!("{flag} needs a value\n{USAGE}"))?;
    raw.parse()
        .map_err(|_| anyhow::anyhow!("invalid value {raw:?} for {flag}\n{USAGE}"))
}

impl Options {
    pub fn from_args(mut args: impl Iterator<Item = String>) -> Result<Self> {
        let mut options = Self {
            mode: Mode::Windowed,
            view_radius: 12,
            vsync: false,
            shading: Shading::Textured,
            screenshot_view: None,
        };
        let mut pending = Vec::new();
        match args.next().as_deref() {
            Some("bench") => {
                options.mode = Mode::Bench(parse_bench(args)?);
                return Ok(options);
            }
            Some("bench-compare") => {
                let mut paths = Vec::new();
                let mut allow_shading_change = false;
                for arg in args {
                    match arg.as_str() {
                        "--allow-shading-change" => allow_shading_change = true,
                        _ => paths.push(PathBuf::from(arg)),
                    }
                }
                let [baseline, candidate] = <[PathBuf; 2]>::try_from(paths).map_err(|_| {
                    anyhow::anyhow!("bench-compare needs two report paths\n{USAGE}")
                })?;
                options.mode = Mode::Compare {
                    baseline,
                    candidate,
                    allow_shading_change,
                };
                return Ok(options);
            }
            Some(first) => pending.push(first.to_owned()),
            None => {}
        }
        let mut args = pending.into_iter().chain(args);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--probe" => options.mode = Mode::Probe,
                "--screenshot" => {
                    options.mode = Mode::Screenshot(next_value::<String>(&mut args, &arg)?.into());
                }
                "--radius" => options.view_radius = next_value(&mut args, &arg)?,
                "--vsync" => options.vsync = true,
                "--shading" => options.shading = parse_shading(&mut args, &arg)?,
                "--scenario" => {
                    let name: String = next_value(&mut args, &arg)?;
                    let scenario = parse_scenario(&name)?;
                    options.view_radius = scenario.view_radius();
                    options.screenshot_view = Some(ScenarioView {
                        scenario,
                        progress: 0.0,
                    });
                }
                "--at" => {
                    let progress: f32 = next_value(&mut args, &arg)?;
                    let view = options
                        .screenshot_view
                        .as_mut()
                        .context("--at needs --scenario first")?;
                    view.progress = progress.clamp(0.0, 1.0).fract();
                }
                "--help" | "-h" => bail!("{USAGE}"),
                _ => bail!("unknown argument {arg:?}\n{USAGE}"),
            }
        }
        Ok(options)
    }
}

fn parse_scenario(name: &str) -> Result<Scenario> {
    Scenario::from_name(name)
        .with_context(|| format!("unknown scenario {name:?}, expected {}", Scenario::NAMES))
}

fn parse_shading(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<Shading> {
    let name: String = next_value(args, flag)?;
    Shading::from_name(&name).with_context(|| format!("unknown shading {name:?}\n{USAGE}"))
}

fn parse_bench(mut args: impl Iterator<Item = String>) -> Result<BenchOptions> {
    let mut bench = BenchOptions::for_scenario(Scenario::Default);
    let mut overrides = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--scenario" => {
                let name: String = next_value(&mut args, &arg)?;
                bench = BenchOptions::for_scenario(parse_scenario(&name)?);
            }
            "--repeats" | "--frames" | "--warmup" | "--threads" => {
                overrides.push((arg.clone(), next_value::<u32>(&mut args, &arg)?));
            }
            "--resolution" => {
                let raw: String = next_value(&mut args, &arg)?;
                let (width, height) = raw.split_once('x').context(USAGE)?;
                bench.width = width.parse().context(USAGE)?;
                bench.height = height.parse().context(USAGE)?;
            }
            "--shading" => bench.shading = parse_shading(&mut args, &arg)?,
            "--out" => bench.out = Some(next_value::<String>(&mut args, &arg)?.into()),
            _ => bail!("unknown bench argument {arg:?}\n{USAGE}"),
        }
    }
    for (flag, value) in overrides {
        match flag.as_str() {
            "--repeats" => bench.repeats = value.max(1),
            "--frames" => bench.measured_frames = value.max(1),
            "--warmup" => bench.warmup_frames = value,
            _ => bench.threads = value.max(1),
        }
    }
    Ok(bench)
}
