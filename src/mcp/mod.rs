use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::router::{prompt::PromptRouter, tool::ToolRouter},
    model::{Implementation, PromptMessage, Role, ServerCapabilities, ServerConfig},
    prompt, prompt_handler, prompt_router, tool, tool_handler, tool_router,
};

#[derive(Clone)]
struct Station {
    project: String,
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

#[tool_router]
impl Station {
    fn new(project: &str) -> Self {
        Self {
            project: project.to_string(),
            tool_router: Self::tool_router(),
            prompt_router: Self::prompt_router(),
        }
    }

    #[tool(description = "Check that the station MCP server is reachable.")]
    async fn ping(&self) -> String {
        format!("pong from station (project {})", self.project)
    }
}

#[prompt_router]
impl Station {
    #[prompt(description = "Call the station ping tool and report the result.")]
    async fn hello(&self) -> Vec<PromptMessage> {
        vec![PromptMessage::new_text(
            Role::User,
            "Call the station ping tool and report what it returned.",
        )]
    }
}

#[tool_handler(router = self.tool_router)]
#[prompt_handler(router = self.prompt_router)]
impl ServerHandler for Station {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new("station", env!("CARGO_PKG_VERSION")))
    }
}

pub fn run(project: &str) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    runtime.block_on(async {
        let service = match Station::new(project).serve(rmcp::transport::stdio()).await {
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
