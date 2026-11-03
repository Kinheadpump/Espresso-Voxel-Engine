use std::process::Command;

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() {
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");
    println!("cargo:rerun-if-env-changed=RUSTFLAGS");
    let commit = command_output("git", &["rev-parse", "--short=12", "HEAD"])
        .unwrap_or_else(|| "unknown".into());
    let dirty = command_output("git", &["status", "--porcelain", "--untracked-files=no"])
        .map_or("unknown".into(), |status| (!status.is_empty()).to_string());
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let rustc_version = command_output(&rustc, &["--version"]).unwrap_or_else(|| "unknown".into());
    let rustflags = std::env::var("CARGO_ENCODED_RUSTFLAGS")
        .unwrap_or_default()
        .replace('\x1f', " ");
    let rustflags = if rustflags.is_empty() {
        "none".to_owned()
    } else {
        rustflags
    };
    println!("cargo:rustc-env=VX_GIT_COMMIT={commit}");
    println!("cargo:rustc-env=VX_GIT_DIRTY={dirty}");
    println!("cargo:rustc-env=VX_RUSTC_VERSION={rustc_version}");
    println!("cargo:rustc-env=VX_RUSTFLAGS={rustflags}");
    println!(
        "cargo:rustc-env=VX_PROFILE={}",
        std::env::var("PROFILE").unwrap_or_default()
    );
}
