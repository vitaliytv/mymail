use std::{
    env,
    process::{Command, ExitCode},
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xtask: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or_else(usage)?;
    if command != "release-assets" {
        return Err(usage());
    }
    let version = args.next().ok_or_else(usage)?;
    if args.next().is_some() || !is_semver(&version) {
        return Err("version must be X.Y.Z, for example: cargo xtask release-assets 0.30.8".into());
    }

    let status = Command::new("bun")
        .args(["--cwd=app", "run", "release:dmg", &version])
        .status()
        .map_err(|error| format!("run local release publisher: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("local release publisher exited with {status}"))
    }
}

fn is_semver(version: &str) -> bool {
    let mut parts = version.split('.');
    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some(major), Some(minor), Some(patch), None)
            if [major, minor, patch]
                .iter()
                .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    )
}

fn usage() -> String {
    "usage: cargo xtask release-assets <X.Y.Z>".into()
}

#[cfg(test)]
mod tests {
    use super::is_semver;

    #[test]
    fn accepts_release_versions_only() {
        assert!(is_semver("0.30.8"));
        assert!(!is_semver("v0.30.8"));
        assert!(!is_semver("0.30"));
    }
}
