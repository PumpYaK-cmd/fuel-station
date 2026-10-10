use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const MARKER: &str = "# station-agent v";

const AGENTS: [(&str, &str); 3] = [
    ("fuel.md", include_str!("agents/fuel.md")),
    ("fuel-high.md", include_str!("agents/fuel-high.md")),
    ("mechanic.md", include_str!("agents/mechanic.md")),
];

pub fn claude_dir() -> PathBuf {
    let base = std::env::var_os("USERPROFILE").expect("USERPROFILE is not set");
    PathBuf::from(base).join(".claude")
}

pub fn agents_dir() -> PathBuf {
    claude_dir().join("agents")
}

pub(super) fn version(text: &str, marker: &str) -> Option<u32> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix(marker))
        .and_then(|n| n.trim().parse().ok())
}

pub(super) fn install_file(path: &Path, bundled: &str, marker: &str) -> io::Result<&'static str> {
    match fs::read_to_string(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir)?;
            }
            fs::write(path, bundled)?;
            Ok("created")
        }
        Err(e) => Err(e),
        Ok(installed) => match (version(&installed, marker), version(bundled, marker)) {
            (Some(have), Some(want)) if have < want => {
                fs::write(path, bundled)?;
                Ok("updated")
            }
            (Some(_), _) => Ok("current"),
            (None, _) => Ok("skipped (user edited)"),
        },
    }
}

pub fn install_agents(agents_dir: &Path) -> io::Result<Vec<(String, &'static str)>> {
    AGENTS
        .iter()
        .map(|(name, bundled)| {
            let action = install_file(&agents_dir.join(name), bundled, MARKER)?;
            Ok((name.to_string(), action))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actions(dir: &Path) -> Vec<&'static str> {
        install_agents(dir)
            .unwrap()
            .into_iter()
            .map(|(_, action)| action)
            .collect()
    }

    #[test]
    fn fresh_dir_creates_all_then_current() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("agents");
        assert_eq!(actions(&dir), ["created"; 3]);
        for (name, bundled) in AGENTS {
            assert_eq!(fs::read_to_string(dir.join(name)).unwrap(), bundled);
        }
        assert_eq!(actions(&dir), ["current"; 3]);
    }

    #[test]
    fn older_marker_is_updated() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        install_agents(dir).unwrap();
        fs::write(dir.join("fuel.md"), "# station-agent v0\nold\n").unwrap();
        assert_eq!(actions(dir), ["updated", "current", "current"]);
        assert_eq!(
            fs::read_to_string(dir.join("fuel.md")).unwrap(),
            AGENTS[0].1
        );
    }

    #[test]
    fn newer_marker_is_current() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        install_agents(dir).unwrap();
        fs::write(dir.join("fuel.md"), "# station-agent v99\nnew\n").unwrap();
        assert_eq!(actions(dir), ["current"; 3]);
        assert_eq!(
            fs::read_to_string(dir.join("fuel.md")).unwrap(),
            "# station-agent v99\nnew\n"
        );
    }

    #[test]
    fn file_without_marker_is_left_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::write(dir.join("mechanic.md"), "my own agent\n").unwrap();
        assert_eq!(
            actions(dir),
            ["created", "created", "skipped (user edited)"]
        );
        assert_eq!(
            fs::read_to_string(dir.join("mechanic.md")).unwrap(),
            "my own agent\n"
        );
    }

    #[test]
    fn bundled_files_have_name_and_marker() {
        for (file, text) in AGENTS {
            let stem = file.trim_end_matches(".md");
            assert!(
                text.lines()
                    .any(|line| line.trim() == format!("name: {stem}")),
                "{file}"
            );
            assert!(version(text, MARKER).is_some(), "{file}");
        }
    }
}
