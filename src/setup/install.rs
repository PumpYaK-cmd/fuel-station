use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const EXE: &str = "fuel-station.exe";
const OLD_EXE: &str = "fuel-station.old.exe";

pub fn app_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|base| PathBuf::from(base).join("FuelStation"))
}

pub fn bin_dir() -> PathBuf {
    app_dir().expect("LOCALAPPDATA is not set").join("bin")
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

pub fn gui_dir() -> PathBuf {
    app_dir().expect("LOCALAPPDATA is not set").join("app")
}

fn install_into(src: &Path, bin: &Path, gui: &Path) -> io::Result<Vec<PathBuf>> {
    let mut installed = vec![install_to(src, bin)?];
    if gui.is_dir() {
        installed.push(install_to(src, gui)?);
    }
    Ok(installed)
}

pub fn install() -> io::Result<Vec<PathBuf>> {
    refuse_redirect()?;
    install_into(&std::env::current_exe()?, &bin_dir(), &gui_dir())
}

fn refuse_redirect() -> io::Result<()> {
    let (Some(local), Some(app)) = (std::env::var_os("LOCALAPPDATA"), app_dir()) else {
        return Ok(());
    };
    fs::create_dir_all(&app)?;
    let probe = format!("probe-{}.tmp", std::process::id());
    fs::write(app.join(&probe), "")?;
    let copy = redirected_copy(Path::new(&local), &app, &probe);
    let _ = fs::remove_file(app.join(&probe));
    let Some(copy) = copy else {
        return Ok(());
    };
    let _ = fs::remove_dir(&app);
    Err(io::Error::other(format!(
        "{} is redirected to {}: this runs inside an MSIX app (Claude desktop) and the folder \
         does not exist yet. Run `fuel-station install` once from a terminal outside Claude.",
        app.display(),
        copy.parent().unwrap_or(&copy).display()
    )))
}

fn redirected_copy(local: &Path, app: &Path, probe: &str) -> Option<PathBuf> {
    let rel = app.strip_prefix(local).ok()?;
    fs::read_dir(local.join("Packages"))
        .ok()?
        .flatten()
        .map(|e| {
            e.path()
                .join("LocalCache")
                .join("Local")
                .join(rel)
                .join(probe)
        })
        .find(|p| p.is_file())
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
    fn install_refreshes_existing_gui_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src.exe");
        write(&src, "v2");
        let (bin, gui) = (tmp.path().join("bin"), tmp.path().join("app"));
        fs::create_dir_all(&gui).unwrap();
        write(&gui.join(EXE), "v1");
        let installed = install_into(&src, &bin, &gui).unwrap();
        assert_eq!(installed, vec![bin.join(EXE), gui.join(EXE)]);
        assert_eq!(fs::read_to_string(gui.join(EXE)).unwrap(), "v2");
    }

    #[test]
    fn install_skips_missing_gui_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src.exe");
        write(&src, "v1");
        let (bin, gui) = (tmp.path().join("bin"), tmp.path().join("app"));
        assert_eq!(install_into(&src, &bin, &gui).unwrap(), vec![bin.join(EXE)]);
        assert!(!gui.exists());
    }

    #[test]
    fn finds_probe_in_package_local_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let local = tmp.path();
        let app = local.join("FuelStation");
        assert_eq!(redirected_copy(local, &app, "p.tmp"), None);
        let cache = local.join("Packages/Claude_x/LocalCache/Local/FuelStation");
        fs::create_dir_all(&cache).unwrap();
        fs::create_dir_all(local.join("Packages/Other_y")).unwrap();
        assert_eq!(redirected_copy(local, &app, "p.tmp"), None);
        write(&cache.join("p.tmp"), "");
        assert_eq!(
            redirected_copy(local, &app, "p.tmp"),
            Some(
                local
                    .join("Packages/Claude_x")
                    .join("LocalCache/Local/FuelStation/p.tmp")
            )
        );
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
