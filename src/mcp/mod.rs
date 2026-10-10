mod tools;

use std::sync::{Arc, Mutex};

use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::router::tool::ToolRouter,
    model::{Implementation, ServerCapabilities, ServerConfig},
    tool_handler,
};

use crate::store::Store;

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

pub fn run(project: &str) {
    let store = match Store::open_default() {
        Ok(store) => store,
        Err(e) => {
            eprintln!("station: cannot open store: {e}");
            std::process::exit(1);
        }
    };
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
