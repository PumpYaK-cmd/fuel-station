mod tools;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::router::tool::ToolRouter,
    model::{Implementation, ServerCapabilities, ServerConfig},
    tool_handler,
};

use crate::store::{Project, Store};

#[derive(Clone)]
struct Station {
    project: String,
    store: Arc<Mutex<Store>>,
    tool_router: ToolRouter<Self>,
}

impl Station {
    fn new(project: &str, store: Store) -> Self {
        Self {
            project: project.to_string(),
            store: Arc::new(Mutex::new(store)),
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Station {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("station", env!("CARGO_PKG_VERSION")))
    }
}

fn normalize(path: &str) -> PathBuf {
    PathBuf::from(
        path.replace('/', "\\")
            .trim_end_matches('\\')
            .to_lowercase(),
    )
}

/// Key of the project whose path is the deepest ancestor of `dir` (or `dir` itself).
fn project_for_dir(projects: &[Project], dir: &Path) -> Option<String> {
    let dir = normalize(&dir.to_string_lossy());
    projects
        .iter()
        .filter_map(|p| Some((p, normalize(p.path.as_deref()?))))
        .filter(|(_, path)| !path.as_os_str().is_empty() && dir.starts_with(path))
        .max_by_key(|(_, path)| path.components().count())
        .map(|(p, _)| p.key.clone())
}

/// `--project` when given, else the project of the working directory. A linked git worktree
/// outside the project folder resolves through its main checkout.
fn resolve_project(store: &Store, flag: Option<&str>) -> Result<String, String> {
    if let Some(key) = flag {
        return Ok(key.to_string());
    }
    let cwd = std::env::current_dir().map_err(|e| format!("no working directory: {e}"))?;
    let projects = store.projects().map_err(|e| e.to_string())?;
    if let Some(key) = project_for_dir(&projects, &cwd) {
        return Ok(key);
    }
    let common = crate::setup::git(
        &cwd,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    );
    if let Some(main) = common.ok().as_deref().map(Path::new).and_then(Path::parent)
        && let Some(key) = project_for_dir(&projects, main)
    {
        return Ok(key);
    }
    Err(format!(
        "no project for {}; pass --project or set the project path in Fuel Station",
        cwd.display()
    ))
}

pub fn run(flag: Option<&str>) {
    let store = match Store::open_default() {
        Ok(store) => store,
        Err(e) => {
            eprintln!("station: cannot open store: {e}");
            std::process::exit(1);
        }
    };
    let project = match resolve_project(&store, flag) {
        Ok(project) => project,
        Err(e) => {
            eprintln!("station: {e}");
            std::process::exit(1);
        }
    };
    let project = project.as_str();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    runtime.block_on(async {
        let service = match Station::new(project, store)
            .serve(rmcp::transport::stdio())
            .await
        {
            Ok(service) => service,
            Err(e) => {
                eprintln!("station: initialize failed: {e}");
                return;
            }
        };
        if let Err(e) = service.waiting().await {
            eprintln!("station: server task failed: {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::ShipMode;

    fn project(key: &str, path: Option<&str>) -> Project {
        Project {
            id: 0,
            key: key.to_string(),
            name: key.to_string(),
            path: path.map(str::to_string),
            ship_mode: ShipMode::Git,
            default_branch: "main".into(),
            verify_cmd: String::new(),
            worktree_dir: String::new(),
            bootstrap_cmd: String::new(),
            ship_cmd: String::new(),
            created_at: 0,
            archived: false,
        }
    }

    fn projects() -> Vec<Project> {
        vec![
            project("YA", Some(r"C:\Dev\YaK Automotive")),
            project("YA2", Some(r"C:\Dev\YaK Aviation 2")),
            project("KP", Some("C:/Dev2/Kitsu Pen")),
            project("NEST", Some(r"C:\Dev\YaK Aviation 2\tools\nested\")),
            project("NONE", None),
            project("EMPTY", Some("")),
        ]
    }

    fn key(dir: &str) -> Option<String> {
        project_for_dir(&projects(), Path::new(dir))
    }

    #[test]
    fn project_dir_and_subdirs_match() {
        assert_eq!(key(r"C:\Dev\YaK Aviation 2").as_deref(), Some("YA2"));
        assert_eq!(
            key(r"C:\Dev\YaK Aviation 2\src\shared").as_deref(),
            Some("YA2")
        );
    }

    #[test]
    fn match_ignores_case_and_separators() {
        assert_eq!(key(r"c:\dev2\kitsu pen\src").as_deref(), Some("KP"));
        assert_eq!(key("C:/Dev/YaK Aviation 2/").as_deref(), Some("YA2"));
    }

    #[test]
    fn sibling_with_shared_prefix_does_not_match() {
        assert_eq!(key(r"C:\Dev\YaK Aviation 2-wt\steer"), None);
        assert_eq!(key(r"C:\Dev\YaK"), None);
    }

    #[test]
    fn deepest_project_wins() {
        assert_eq!(
            key(r"C:\Dev\YaK Aviation 2\tools\nested\x").as_deref(),
            Some("NEST")
        );
    }

    #[test]
    fn projects_without_a_path_never_match() {
        assert_eq!(key(r"C:\"), None);
    }
}
