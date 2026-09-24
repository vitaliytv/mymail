use std::{
    env, fs,
    process::{Command, ExitCode},
};

const MANIFEST_PATH: &str = "app/package.json";

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

    // Versions are bumped only by the release PR; this command never changes the repository.
    require_manifest_version(&version)?;

    let status = Command::new("bun")
        .args(["--cwd=app", "run", "release:dmg", &version])
        .status()
        .map_err(|error| format!("run local release builder: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("local release builder exited with {status}"))
    }
}

fn require_manifest_version(version: &str) -> Result<(), String> {
    let manifest = fs::read_to_string(MANIFEST_PATH)
        .map_err(|error| format!("read {MANIFEST_PATH}: {error}"))?;
    let current_version = manifest_version(&manifest)?;
    if current_version == version {
        Ok(())
    } else {
        Err(format!(
            "{MANIFEST_PATH} has version {current_version}; check out the merged release tag v{version} first"
        ))
    }
}

fn manifest_version(manifest: &str) -> Result<&str, String> {
    let marker = "\"version\": \"";
    let start = manifest
        .find(marker)
        .map(|index| index + marker.len())
        .ok_or_else(|| format!("{MANIFEST_PATH} has no version field"))?;
    let end = manifest[start..]
        .find('"')
        .map(|index| start + index)
        .ok_or_else(|| format!("{MANIFEST_PATH} version is unterminated"))?;
    Ok(&manifest[start..end])
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
    use super::{is_semver, manifest_version};

    #[test]
    fn accepts_release_versions_only() {
        assert!(is_semver("0.30.8"));
        assert!(!is_semver("v0.30.8"));
        assert!(!is_semver("0.30"));
    }

    #[test]
    fn reads_package_manifest_version() {
        assert_eq!(
            manifest_version("{\"name\": \"app\", \"version\": \"0.30.9\"}"),
            Ok("0.30.9")
        );
    }
}
