use std::{
    env, fs,
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

    prepare_release(&version)?;

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

fn prepare_release(version: &str) -> Result<(), String> {
    ensure_clean_worktree()?;

    let config_path = "app/src-tauri/tauri.conf.json";
    let config =
        fs::read_to_string(config_path).map_err(|error| format!("read {config_path}: {error}"))?;
    let current_version = config_version(&config)?;

    if current_version != version {
        let updated = config.replacen(
            &format!("\"version\": \"{current_version}\""),
            &format!("\"version\": \"{version}\""),
            1,
        );
        fs::write(config_path, updated).map_err(|error| format!("write {config_path}: {error}"))?;
        run_command(Command::new("git").args(["add", "app/src-tauri/tauri.conf.json"]))?;
        run_command(Command::new("git").args([
            "commit",
            "-m",
            &format!("chore: release v{version}"),
        ]))?;
    }

    let tag = format!("v{version}");
    if tag_exists(&tag)? {
        let tag_commit = git_output(&["rev-parse", &format!("{tag}^{{commit}}")])?;
        let head_commit = git_output(&["rev-parse", "HEAD"])?;
        if tag_commit != head_commit {
            return Err(format!(
                "{tag} already points at {tag_commit}, not HEAD {head_commit}"
            ));
        }
    } else {
        run_command(Command::new("git").args([
            "tag",
            "-a",
            &tag,
            "-m",
            &format!("Release {tag}"),
        ]))?;
    }

    Ok(())
}

fn ensure_clean_worktree() -> Result<(), String> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .map_err(|error| format!("inspect worktree: {error}"))?;
    if !output.status.success() {
        return Err("inspect worktree: git status failed".into());
    }
    if output.stdout.is_empty() {
        Ok(())
    } else {
        Err("worktree must be clean before preparing a release".into())
    }
}

fn config_version(config: &str) -> Result<&str, String> {
    let marker = "\"version\": \"";
    let start = config
        .find(marker)
        .map(|index| index + marker.len())
        .ok_or_else(|| "tauri config has no version field".to_owned())?;
    let end = config[start..]
        .find('"')
        .map(|index| start + index)
        .ok_or_else(|| "tauri config version is unterminated".to_owned())?;
    Ok(&config[start..end])
}

fn tag_exists(tag: &str) -> Result<bool, String> {
    let status = Command::new("git")
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/tags/{tag}"),
        ])
        .status()
        .map_err(|error| format!("check tag {tag}: {error}"))?;
    Ok(status.success())
}

fn git_output(args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|error| format!("run git {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "git {} exited with {}",
            args.join(" "),
            output.status
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| format!("read git output: {error}"))
}

fn run_command(command: &mut Command) -> Result<(), String> {
    let program = command.get_program().to_string_lossy().into_owned();
    let status = command
        .status()
        .map_err(|error| format!("run {program}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} exited with {status}"))
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
    use super::{config_version, is_semver};

    #[test]
    fn accepts_release_versions_only() {
        assert!(is_semver("0.30.8"));
        assert!(!is_semver("v0.30.8"));
        assert!(!is_semver("0.30"));
    }

    #[test]
    fn reads_tauri_config_version() {
        assert_eq!(config_version("{\"version\": \"0.30.9\"}"), Ok("0.30.9"));
    }
}
