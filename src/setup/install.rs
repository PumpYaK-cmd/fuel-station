use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const EXE: &str = "fuel-station.exe";
const OLD_EXE: &str = "fuel-station.old.exe";

pub fn bin_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA is not set");
    PathBuf::from(base).join("FuelStation").join("bin")
}

pub fn installed_exe() -> PathBuf {
    bin_dir().join(EXE)
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

pub fn install_to(src: &Path, dir: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let dest = dir.join(EXE);
    if same_file(src, &dest) {
        return Ok(dest);
    }
    let _ = fs::remove_file(dir.join(OLD_EXE));
    if dest.exists() {
        if fs::copy(src, &dest).is_ok() {
            return Ok(dest);
        }
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let aside = if dir.join(OLD_EXE).exists() {
            dir.join(format!("fuel-station.old-{secs}.exe"))
        } else {
            dir.join(OLD_EXE)
        };
        fs::rename(&dest, &aside)?;
        if let Err(e) = fs::copy(src, &dest) {
            let _ = fs::rename(&aside, &dest);
            return Err(e);
        }
        return Ok(dest);
    }
    fs::copy(src, &dest)?;
    Ok(dest)
}

pub fn install() -> io::Result<PathBuf> {
    install_to(&std::env::current_exe()?, &bin_dir())
}

pub fn cleanup_old(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("fuel-station.old") && name.ends_with(".exe") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, content: &str) {
        fs::write(path, content).unwrap();
    }

    #[test]
    fn fresh_install_copies() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src.exe");
        write(&src, "v1");
        let dir = tmp.path().join("bin");
        let dest = install_to(&src, &dir).unwrap();
        assert_eq!(dest, dir.join(EXE));
        assert_eq!(fs::read_to_string(dest).unwrap(), "v1");
    }

    #[test]
    fn reinstall_replaces_content() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src.exe");
        let dir = tmp.path().join("bin");
        write(&src, "v1");
        install_to(&src, &dir).unwrap();
        write(&src, "v2");
        let dest = install_to(&src, &dir).unwrap();
        assert_eq!(fs::read_to_string(dest).unwrap(), "v2");
    }

    #[test]
    fn src_equal_dest_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("bin");
        fs::create_dir_all(&dir).unwrap();
        let dest = dir.join(EXE);
        write(&dest, "v1");
        assert_eq!(install_to(&dest, &dir).unwrap(), dest);
        assert_eq!(fs::read_to_string(dest).unwrap(), "v1");
    }

    #[test]
    fn cleanup_removes_old_files_only() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write(&dir.join(EXE), "keep");
        write(&dir.join(OLD_EXE), "x");
        write(&dir.join("fuel-station.old-123.exe"), "x");
        cleanup_old(dir);
        assert!(dir.join(EXE).exists());
        assert!(!dir.join(OLD_EXE).exists());
        assert!(!dir.join("fuel-station.old-123.exe").exists());
    }
}
