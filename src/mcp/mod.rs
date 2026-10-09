mod tools;

use std::sync::{Arc, Mutex};

use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{
        router::{prompt::PromptRouter, tool::ToolRouter},
        wrapper::Parameters,
    },
    model::{Implementation, PromptMessage, Role, ServerCapabilities, ServerConfig},
    prompt, prompt_handler, prompt_router, tool_handler,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::store::Store;

#[derive(Clone)]
struct Station {
    project: String,
    store: Arc<Mutex<Store>>,
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

impl Station {
    fn new(project: &str, store: Store) -> Self {
        Self {
            project: project.to_string(),
            store: Arc::new(Mutex::new(store)),
            tool_router: Self::tool_router(),
            prompt_router: Self::prompt_router(),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
struct GrabArgs {
    /// Card id, e.g. FS-3; omit for the top Ready card.
    card: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct FarmArgs {
    /// How many cards to batch; omit for the Ready list.
    n: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct CardArgs {
    /// One sentence describing the work.
    text: String,
}

fn prompt_text(body: &str, arg: Option<&str>) -> Vec<PromptMessage> {
    let arg = arg
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .unwrap_or("none");
    vec![PromptMessage::new_text(
        Role::User,
        format!("{body}\nArgument: {arg}"),
    )]
}

#[prompt_router]
impl Station {
    #[prompt(
        name = "grab",
        description = "Ship one card end to end: claim, plan, implement, review, ship."
    )]
    async fn grab_prompt(&self, Parameters(a): Parameters<GrabArgs>) -> Vec<PromptMessage> {
        prompt_text(include_str!("prompts/grab.md"), a.card.as_deref())
    }

    #[prompt(
        name = "farm",
        description = "Plan a batch of Ready cards and run one Fuel agent per card."
    )]
    async fn farm_prompt(&self, Parameters(a): Parameters<FarmArgs>) -> Vec<PromptMessage> {
        prompt_text(include_str!("prompts/farm.md"), a.n.as_deref())
    }

    #[prompt(
        name = "review",
        description = "Spawn a cold Mechanic review of the current card's diff."
    )]
    async fn review_prompt(&self) -> Vec<PromptMessage> {
        prompt_text(include_str!("prompts/review.md"), None)
    }

    #[prompt(name = "card", description = "File one Backlog card from a sentence.")]
    async fn card_prompt(&self, Parameters(a): Parameters<CardArgs>) -> Vec<PromptMessage> {
        prompt_text(include_str!("prompts/card.md"), Some(&a.text))
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

#[cfg(test)]
mod tests {
    use rmcp::model::ContentBlock;

    use super::*;

    fn station() -> (tempfile::TempDir, Station) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("station.db")).unwrap();
        (dir, Station::new("FS", store))
    }

    fn text(messages: Vec<PromptMessage>) -> String {
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, Role::User);
        match &messages[0].content {
            ContentBlock::Text(t) => t.text.clone(),
            _ => panic!("expected text content"),
        }
    }

    #[tokio::test]
    async fn prompts_end_with_their_argument() {
        let (_dir, s) = station();
        let grab = |card: Option<&str>| {
            s.grab_prompt(Parameters(GrabArgs {
                card: card.map(str::to_string),
            }))
        };
        let cases = [
            (text(grab(Some("FS-3")).await), "FS-3"),
            (text(grab(None).await), "none"),
            (text(grab(Some(" ")).await), "none"),
            (
                text(
                    s.farm_prompt(Parameters(FarmArgs {
                        n: Some("4".to_string()),
                    }))
                    .await,
                ),
                "4",
            ),
            (text(s.review_prompt().await), "none"),
            (
                text(
                    s.card_prompt(Parameters(CardArgs {
                        text: "Fix the badge".to_string(),
                    }))
                    .await,
                ),
                "Fix the badge",
            ),
        ];
        for (body, arg) in cases {
            let line = format!("\nArgument: {arg}");
            assert!(body.ends_with(&line), "missing {line:?}");
            assert!(body.len() > line.len() + 100);
            let lower = body.to_lowercase();
            assert!(!lower.contains("trello"));
            assert!(!lower.contains("plans/"));
            assert!(!body.contains('\u{2014}'));
        }
    }
}
