use std::fs;
use std::path::Path;

use super::{git, in_work_tree};
use crate::store::{NewProject, ShipMode, derive_key};

const CARGO_GATE: &str =
    "cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test";

#[derive(Debug, Clone, Default)]
pub struct Facts {
    pub is_git: bool,
    pub remote: Option<String>,
    pub has_claude_md: bool,
}

pub fn detect(dir: &Path, taken_keys: &[String]) -> NewProject {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.display().to_string());
    let default_branch = match git(dir, &["symbolic-ref", "--short", "HEAD"]) {
        Ok(branch) if !branch.is_empty() => branch,
        _ => "main".to_string(),
    };
    let (verify_cmd, ship_mode) = gate(dir);
    let worktree_dir = match ship_mode {
        ShipMode::Git => format!("../{name}-wt"),
        ShipMode::Studio => String::new(),
    };
    NewProject {
        key: derive_key(&name, taken_keys),
        name,
        path: Some(dir.display().to_string()),
        ship_mode,
        default_branch,
        verify_cmd,
        worktree_dir,
        bootstrap_cmd: String::new(),
        ship_cmd: String::new(),
    }
}

pub fn facts(dir: &Path) -> Facts {
    let is_git = in_work_tree(dir);
    Facts {
        is_git,
        remote: is_git
            .then(|| git(dir, &["remote", "get-url", "origin"]).ok())
            .flatten()
            .filter(|url| !url.is_empty()),
        has_claude_md: dir.join("CLAUDE.md").is_file(),
    }
}

fn gate(dir: &Path) -> (String, ShipMode) {
    if dir.join("Cargo.toml").is_file() {
        return (CARGO_GATE.to_string(), ShipMode::Git);
    }
    if dir.join("default.project.json").is_file() || has_rojo_project(dir) {
        return (String::new(), ShipMode::Studio);
    }
    if dir.join("package.json").is_file() {
        return (npm_gate(dir), ShipMode::Git);
    }
    if dir.join("pyproject.toml").is_file() {
        return ("py -3 -m pytest".to_string(), ShipMode::Git);
    }
    (String::new(), ShipMode::Git)
}

fn has_rojo_project(dir: &Path) -> bool {
    fs::read_dir(dir).is_ok_and(|entries| {
        entries
            .flatten()
            .any(|e| e.file_name().to_string_lossy().ends_with(".project.json"))
    })
}

fn npm_gate(dir: &Path) -> String {
    let scripts = fs::read_to_string(dir.join("package.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("scripts").cloned());
    let has = |name: &str| scripts.as_ref().is_some_and(|s| s.get(name).is_some());
    if has("lint") && has("test") {
        "npm run lint && npm test".to_string()
    } else if has("test") {
        "npm test".to_string()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_with(files: &[(&str, &str)]) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        for (name, content) in files {
            fs::write(tmp.path().join(name), content).unwrap();
        }
        tmp
    }

    #[test]
    fn cargo_project() {
        let tmp = dir_with(&[("Cargo.toml", "")]);
        let p = detect(tmp.path(), &[]);
        assert_eq!(p.verify_cmd, CARGO_GATE);
        assert_eq!(p.ship_mode, ShipMode::Git);
        let folder = tmp.path().file_name().unwrap().to_string_lossy();
        assert_eq!(p.name, folder);
        assert_eq!(p.worktree_dir, format!("../{folder}-wt"));
        assert_eq!(
            p.path.as_deref(),
            Some(tmp.path().display().to_string().as_str())
        );
        assert_eq!(p.default_branch, "main");
    }

    #[test]
    fn default_rojo_project_is_studio() {
        let tmp = dir_with(&[("default.project.json", "{}"), ("package.json", "{}")]);
        let p = detect(tmp.path(), &[]);
        assert_eq!(p.ship_mode, ShipMode::Studio);
        assert_eq!(p.verify_cmd, "");
        assert_eq!(p.worktree_dir, "");
    }

    #[test]
    fn named_rojo_project_is_studio() {
        let tmp = dir_with(&[("place.project.json", "{}")]);
        assert_eq!(detect(tmp.path(), &[]).ship_mode, ShipMode::Studio);
    }

    #[test]
    fn npm_scripts_pick_gate() {
        let both = dir_with(&[("package.json", r#"{"scripts":{"lint":"x","test":"y"}}"#)]);
        assert_eq!(
            detect(both.path(), &[]).verify_cmd,
            "npm run lint && npm test"
        );
        let test = dir_with(&[("package.json", r#"{"scripts":{"test":"y"}}"#)]);
        assert_eq!(detect(test.path(), &[]).verify_cmd, "npm test");
        let none = dir_with(&[("package.json", r#"{"scripts":{"lint":"x"}}"#)]);
        let p = detect(none.path(), &[]);
        assert_eq!(p.verify_cmd, "");
        assert_eq!(p.ship_mode, ShipMode::Git);
    }

    #[test]
    fn pyproject_gate() {
        let tmp = dir_with(&[("pyproject.toml", "")]);
        assert_eq!(detect(tmp.path(), &[]).verify_cmd, "py -3 -m pytest");
    }

    #[test]
    fn empty_folder_defaults() {
        let tmp = dir_with(&[]);
        let p = detect(tmp.path(), &[]);
        assert_eq!(p.verify_cmd, "");
        assert_eq!(p.ship_mode, ShipMode::Git);
        let f = facts(tmp.path());
        assert!(!f.is_git && f.remote.is_none() && !f.has_claude_md);
    }

    #[test]
    fn key_avoids_taken() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("fuel-station");
        fs::create_dir(&dir).unwrap();
        assert_eq!(detect(&dir, &["FS".to_string()]).key, "FS2");
    }

    #[test]
    fn git_branch_and_facts() {
        let tmp = dir_with(&[("CLAUDE.md", "x")]);
        let dir = tmp.path();
        git(dir, &["init", "-b", "trunk"]).unwrap();
        git(
            dir,
            &["remote", "add", "origin", "https://example.com/r.git"],
        )
        .unwrap();
        assert_eq!(detect(dir, &[]).default_branch, "trunk");
        let f = facts(dir);
        assert!(f.is_git && f.has_claude_md);
        assert_eq!(f.remote.as_deref(), Some("https://example.com/r.git"));
    }
}
