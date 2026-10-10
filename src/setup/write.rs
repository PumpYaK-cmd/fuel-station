use std::fs;
use std::io;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

#[cfg(windows)]
use super::CREATE_NO_WINDOW;
use super::{git, in_work_tree};
use crate::store::{Project, ShipMode};

const BLOCK_START: &str = "<!-- station -->";
const BLOCK_END: &str = "<!-- /station -->";
pub const LOCAL_MD: &str = "CLAUDE.local.md";

#[derive(Debug, Clone)]
pub struct Written {
    pub path: PathBuf,
    pub action: String,
}

impl Written {
    fn new(path: PathBuf, action: impl Into<String>) -> Written {
        Written {
            path,
            action: action.into(),
        }
    }
}

pub fn apply(p: &Project, exe: &Path, local: bool) -> Vec<Written> {
    let Some(dir) = p.path.as_deref().filter(|s| !s.is_empty()).map(Path::new) else {
        return vec![Written::new(PathBuf::new(), "error: project has no path")];
    };
    if local {
        let mut out = vec![mcp_local(dir, &p.key, exe)];
        if git(dir, &["ls-files", "--error-unmatch", LOCAL_MD]).is_ok() {
            out.push(Written::new(
                dir.join(LOCAL_MD),
                "error: tracked by git, not edited",
            ));
            return out;
        }
        out.push(claude_md(dir, LOCAL_MD, &p.key));
        if in_work_tree(dir) {
            out.push(exclude_local_md(dir));
        }
        return out;
    }
    let mut out = vec![
        mcp_json(dir, &p.key, exe),
        claude_md(dir, "CLAUDE.md", &p.key),
    ];
    if p.ship_mode == ShipMode::Git && !in_work_tree(dir) {
        git_init(dir, &p.default_branch, &mut out);
    }
    out
}

fn read_existing(path: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

fn save(path: PathBuf, existing: Option<&str>, content: &str) -> Written {
    if existing == Some(content) {
        return Written::new(path, "unchanged");
    }
    match fs::write(&path, content) {
        Ok(()) if existing.is_some() => Written::new(path, "updated"),
        Ok(()) => Written::new(path, "created"),
        Err(e) => Written::new(path, format!("error: {e}")),
    }
}

fn mcp_json(dir: &Path, key: &str, exe: &Path) -> Written {
    let path = dir.join(".mcp.json");
    let existing = match read_existing(&path) {
        Ok(s) => s,
        Err(e) => return Written::new(path, format!("error: {e}")),
    };
    let mut root: Value = match existing.as_deref() {
        None => json!({}),
        Some(s) => match serde_json::from_str(s) {
            Ok(v) => v,
            Err(e) => return Written::new(path, format!("error: invalid JSON: {e}")),
        },
    };
    let Some(top) = root.as_object_mut() else {
        return Written::new(path, "error: top level is not an object");
    };
    let Some(servers) = top
        .entry("mcpServers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
    else {
        return Written::new(path, "error: mcpServers is not an object");
    };
    let entry = json!({
        "type": "stdio",
        "command": exe.display().to_string(),
        "args": ["mcp", "--project", key],
    });
    if existing.is_some() && servers.get("station") == Some(&entry) {
        return Written::new(path, "unchanged");
    }
    servers.insert("station".to_string(), entry);
    let content = match serde_json::to_string_pretty(&root) {
        Ok(s) => s + "\n",
        Err(e) => return Written::new(path, format!("error: {e}")),
    };
    save(path, existing.as_deref(), &content)
}

fn mcp_local(dir: &Path, key: &str, exe: &Path) -> Written {
    let path = PathBuf::from("claude mcp add --scope local station");
    let exe = exe.display().to_string();
    let args = [
        "mcp",
        "add",
        "--scope",
        "local",
        "station",
        "--",
        &exe,
        "mcp",
        "--project",
        key,
    ];
    let registered = claude(dir, &args).or_else(|add_err| {
        claude(dir, &["mcp", "remove", "--scope", "local", "station"]).map_err(|_| add_err)?;
        claude(dir, &args)
    });
    match registered {
        Ok(()) => Written::new(path, "registered"),
        Err(e) => Written::new(path, format!("error: {e}")),
    }
}

fn claude(dir: &Path, args: &[&str]) -> Result<(), String> {
    let mut command = Command::new("claude");
    command.current_dir(dir).args(args);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let output = command.output().map_err(|e| format!("claude: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn exclude_local_md(dir: &Path) -> Written {
    let path = match git(dir, &["rev-parse", "--git-path", "info/exclude"]) {
        Ok(p) => dir.join(p),
        Err(e) => return Written::new(dir.join(".git"), format!("error: {e}")),
    };
    let existing = match read_existing(&path) {
        Ok(s) => s,
        Err(e) => return Written::new(path, format!("error: {e}")),
    };
    if git(dir, &["check-ignore", "-q", LOCAL_MD]).is_ok() {
        return Written::new(path, "unchanged");
    }
    let content = match existing.as_deref() {
        None | Some("") => format!("{LOCAL_MD}\n"),
        Some(text) if text.ends_with('\n') => format!("{text}{LOCAL_MD}\n"),
        Some(text) => format!("{text}\n{LOCAL_MD}\n"),
    };
    if let Some(parent) = path.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        return Written::new(path, format!("error: {e}"));
    }
    save(path, existing.as_deref(), &content)
}

fn claude_md(dir: &Path, file: &str, key: &str) -> Written {
    let path = dir.join(file);
    let existing = match read_existing(&path) {
        Ok(s) => s,
        Err(e) => return Written::new(path, format!("error: {e}")),
    };
    let block = format!(
        "{BLOCK_START}\nStation project `{key}`. Use the station MCP tools for the board; \
         runbook is `/grab`.\n{BLOCK_END}"
    );
    let content = match existing.as_deref() {
        None | Some("") => format!("{block}\n"),
        Some(text) if text.contains(BLOCK_START) && block_range(text).is_none() => {
            return Written::new(path, format!("error: {BLOCK_START} without {BLOCK_END}"));
        }
        Some(text) => match block_range(text) {
            Some((start, end)) => format!("{}{block}{}", &text[..start], &text[end..]),
            None if text.ends_with('\n') => format!("{text}\n{block}\n"),
            None => format!("{text}\n\n{block}\n"),
        },
    };
    save(path, existing.as_deref(), &content)
}

fn block_range(text: &str) -> Option<(usize, usize)> {
    let start = text.find(BLOCK_START)?;
    let end = start + text[start..].find(BLOCK_END)? + BLOCK_END.len();
    Some((start, end))
}

fn git_init(dir: &Path, branch: &str, out: &mut Vec<Written>) {
    let branch = if branch.is_empty() { "main" } else { branch };
    if let Err(e) = git(dir, &["init", "-b", branch]) {
        out.push(Written::new(dir.join(".git"), format!("error: {e}")));
        return;
    }
    out.push(Written::new(dir.join(".git"), "created"));
    let ignore = if dir.join("Cargo.toml").is_file() {
        Some("/target\n")
    } else if dir.join("package.json").is_file() {
        Some("node_modules/\n")
    } else {
        None
    };
    let ignore_path = dir.join(".gitignore");
    if let Some(ignore) = ignore.filter(|_| !ignore_path.exists()) {
        let written = save(ignore_path, None, ignore);
        let failed = written.action.starts_with("error");
        out.push(written);
        if failed {
            return;
        }
    }
    if let Err(e) = git(dir, &["add", "-A"]) {
        out.push(Written::new(
            dir.to_path_buf(),
            format!("error: git add: {e}"),
        ));
        return;
    }
    out.push(Written::new(dir.to_path_buf(), "staged"));
    match git(dir, &["commit", "-m", "Initial commit"]) {
        Ok(_) => out.push(Written::new(dir.to_path_buf(), "committed")),
        Err(e) => out.push(Written::new(
            dir.to_path_buf(),
            format!("error: git commit: {e}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(dir: &Path, ship_mode: ShipMode) -> Project {
        Project {
            id: 1,
            key: "FS".to_string(),
            name: "Fuel Station".to_string(),
            path: Some(dir.display().to_string()),
            ship_mode,
            default_branch: "main".to_string(),
            verify_cmd: String::new(),
            worktree_dir: String::new(),
            bootstrap_cmd: String::new(),
            ship_cmd: String::new(),
            created_at: 0,
            archived: false,
        }
    }

    fn actions(written: &[Written]) -> Vec<&str> {
        written.iter().map(|w| w.action.as_str()).collect()
    }

    const EXE: &str = r"C:\bin\fuel-station.exe";

    #[test]
    fn fresh_then_rerun_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let p = project(tmp.path(), ShipMode::Studio);
        assert_eq!(
            actions(&apply(&p, Path::new(EXE), false)),
            ["created", "created"]
        );
        let mcp: Value =
            serde_json::from_str(&fs::read_to_string(tmp.path().join(".mcp.json")).unwrap())
                .unwrap();
        assert_eq!(
            mcp["mcpServers"]["station"],
            json!({"type": "stdio", "command": EXE, "args": ["mcp", "--project", "FS"]})
        );
        let md = fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
        assert!(md.starts_with(BLOCK_START) && md.contains("`FS`"));
        assert_eq!(
            actions(&apply(&p, Path::new(EXE), false)),
            ["unchanged", "unchanged"]
        );
    }

    #[test]
    fn keeps_other_servers() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join(".mcp.json"),
            r#"{"mcpServers":{"other":{"command":"x"}},"extra":1}"#,
        )
        .unwrap();
        let p = project(tmp.path(), ShipMode::Studio);
        assert_eq!(apply(&p, Path::new(EXE), false)[0].action, "updated");
        let mcp: Value =
            serde_json::from_str(&fs::read_to_string(tmp.path().join(".mcp.json")).unwrap())
                .unwrap();
        assert_eq!(mcp["mcpServers"]["other"], json!({"command": "x"}));
        assert_eq!(mcp["extra"], json!(1));
        assert_eq!(mcp["mcpServers"]["station"]["args"][2], "FS");
    }

    #[test]
    fn replaces_only_station_block() {
        let tmp = tempfile::tempdir().unwrap();
        let old = format!("# Top\n\n{BLOCK_START}\nold text\n{BLOCK_END}\n\n## Tail\n");
        fs::write(tmp.path().join("CLAUDE.md"), &old).unwrap();
        let p = project(tmp.path(), ShipMode::Studio);
        assert_eq!(apply(&p, Path::new(EXE), false)[1].action, "updated");
        let md = fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
        assert!(md.starts_with("# Top\n\n<!-- station -->\nStation project `FS`."));
        assert!(md.ends_with("<!-- /station -->\n\n## Tail\n"));
        assert!(!md.contains("old text"));
    }

    #[test]
    fn appends_block_after_blank_line() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("CLAUDE.md"), "# Rules").unwrap();
        let p = project(tmp.path(), ShipMode::Studio);
        apply(&p, Path::new(EXE), false);
        let md = fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
        assert!(md.starts_with("# Rules\n\n<!-- station -->\n"));
        assert!(md.ends_with("<!-- /station -->\n"));
    }

    #[test]
    fn invalid_mcp_json_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join(".mcp.json"), "{ nope").unwrap();
        let p = project(tmp.path(), ShipMode::Studio);
        assert!(
            apply(&p, Path::new(EXE), false)[0]
                .action
                .starts_with("error")
        );
        assert_eq!(
            fs::read_to_string(tmp.path().join(".mcp.json")).unwrap(),
            "{ nope"
        );
    }

    #[test]
    fn unclosed_block_is_error() {
        let tmp = tempfile::tempdir().unwrap();
        let old = format!(
            "{BLOCK_START}
mine
"
        );
        fs::write(tmp.path().join("CLAUDE.md"), &old).unwrap();
        let p = project(tmp.path(), ShipMode::Studio);
        assert!(
            apply(&p, Path::new(EXE), false)[1]
                .action
                .starts_with("error")
        );
        assert_eq!(
            fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap(),
            old
        );
    }

    #[test]
    fn subfolder_of_repo_is_not_initialised() {
        let tmp = tempfile::tempdir().unwrap();
        git(tmp.path(), &["init", "-b", "main"]).unwrap();
        let sub = tmp.path().join("game");
        fs::create_dir(&sub).unwrap();
        let p = project(&sub, ShipMode::Git);
        assert_eq!(
            actions(&apply(&p, Path::new(EXE), false)),
            ["created", "created"]
        );
        assert!(!sub.join(".git").exists());
    }

    #[test]
    fn no_path_is_error() {
        let tmp = tempfile::tempdir().unwrap();
        let mut p = project(tmp.path(), ShipMode::Git);
        p.path = None;
        assert!(
            apply(&p, Path::new(EXE), false)[0]
                .action
                .starts_with("error")
        );
    }

    #[test]
    fn local_block_goes_to_claude_local_md() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("CLAUDE.md"), "# Team rules\n").unwrap();
        assert_eq!(claude_md(tmp.path(), LOCAL_MD, "HC").action, "created");
        let local = fs::read_to_string(tmp.path().join(LOCAL_MD)).unwrap();
        assert!(local.starts_with(BLOCK_START) && local.contains("`HC`"));
        assert_eq!(
            fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap(),
            "# Team rules\n"
        );
    }

    #[test]
    fn local_md_excluded_once() {
        let tmp = tempfile::tempdir().unwrap();
        git(tmp.path(), &["init", "-b", "main"]).unwrap();
        let exclude = tmp.path().join(".git/info/exclude");
        fs::write(&exclude, "# keep").unwrap();
        assert_eq!(exclude_local_md(tmp.path()).action, "updated");
        assert_eq!(
            fs::read_to_string(&exclude).unwrap(),
            format!("# keep\n{LOCAL_MD}\n")
        );
        assert_eq!(exclude_local_md(tmp.path()).action, "unchanged");
    }

    #[test]
    fn local_md_already_gitignored_is_left() {
        let tmp = tempfile::tempdir().unwrap();
        git(tmp.path(), &["init", "-b", "main"]).unwrap();
        fs::write(tmp.path().join(".gitignore"), format!("{LOCAL_MD}\n")).unwrap();
        let exclude = tmp.path().join(".git/info/exclude");
        let before = fs::read_to_string(&exclude).ok();
        assert_eq!(exclude_local_md(tmp.path()).action, "unchanged");
        assert_eq!(fs::read_to_string(&exclude).ok(), before);
    }

    #[test]
    fn git_mode_inits_repo_with_one_commit() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        for var in ["GIT_AUTHOR", "GIT_COMMITTER"] {
            // SAFETY: Windows env access is locked; no test reads these vars.
            unsafe {
                std::env::set_var(format!("{var}_NAME"), "t");
                std::env::set_var(format!("{var}_EMAIL"), "t@t");
            }
        }
        let p = project(tmp.path(), ShipMode::Git);
        let written = apply(&p, Path::new(EXE), false);
        assert_eq!(
            actions(&written),
            [
                "created",
                "created",
                "created",
                "created",
                "staged",
                "committed"
            ],
            "{written:?}"
        );
        assert_eq!(
            fs::read_to_string(tmp.path().join(".gitignore")).unwrap(),
            "/target\n"
        );
        assert_eq!(
            git(tmp.path(), &["rev-list", "--count", "HEAD"]).unwrap(),
            "1"
        );
        assert_eq!(
            git(tmp.path(), &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap(),
            "main"
        );
        assert_eq!(
            actions(&apply(&p, Path::new(EXE), false)),
            ["unchanged", "unchanged"]
        );
    }
}
