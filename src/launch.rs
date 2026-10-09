use std::path::Path;
use std::process::Stdio;

use serde_json::Value;

#[cfg(windows)]
use crate::setup::CREATE_NO_WINDOW;

pub const RUN_BUDGET_USD: f64 = 10.0;

pub fn command(dir: &Path, card_key: &str) -> tokio::process::Command {
    let mut command = tokio::process::Command::new("claude");
    command
        .arg("-p")
        .arg(format!("/mcp__station__grab {card_key}"))
        .args([
            "--output-format",
            "stream-json",
            "--verbose",
            "--permission-mode",
            "auto",
            "--max-budget-usd",
        ])
        .arg(RUN_BUDGET_USD.to_string());
    let mcp = dir.join(".mcp.json");
    if mcp.is_file() {
        command.arg("--mcp-config").arg(mcp);
    }
    command
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    Text(String),
    Done {
        subtype: String,
        cost: Option<f64>,
        turns: Option<i64>,
    },
}

pub fn parse(line: &str) -> Vec<Line> {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return vec![Line::Text(line.to_string())];
    };
    match value["type"].as_str() {
        Some("system") if value["subtype"] == "init" => {
            vec![Line::Text("session started".to_string())]
        }
        Some("assistant") => blocks(&value)
            .iter()
            .flat_map(|block| match block["type"].as_str() {
                Some("text") => block["text"]
                    .as_str()
                    .unwrap_or_default()
                    .lines()
                    .map(|l| Line::Text(l.to_string()))
                    .collect(),
                Some("tool_use") => vec![Line::Text(format!(
                    "→ {} {}",
                    block["name"].as_str().unwrap_or_default(),
                    first(&block["input"].to_string(), 80)
                ))],
                _ => Vec::new(),
            })
            .collect(),
        Some("user") => blocks(&value)
            .iter()
            .filter(|block| block["type"] == "tool_result" && block["is_error"] == true)
            .map(|block| {
                let content = match &block["content"] {
                    Value::String(s) => s.clone(),
                    Value::Array(parts) => parts
                        .iter()
                        .filter_map(|p| p["text"].as_str())
                        .collect::<Vec<_>>()
                        .join(" "),
                    other => other.to_string(),
                };
                Line::Text(format!("← error {}", first(&content, 120)))
            })
            .collect(),
        Some("result") => vec![Line::Done {
            subtype: value["subtype"].as_str().unwrap_or_default().to_string(),
            cost: value["total_cost_usd"].as_f64(),
            turns: value["num_turns"].as_i64(),
        }],
        _ => Vec::new(),
    }
}

fn blocks(value: &Value) -> &[Value] {
    value["message"]["content"]
        .as_array()
        .map_or(&[], Vec::as_slice)
}

fn first(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

pub fn kill_tree(pid: u32) {
    let mut command = std::process::Command::new("taskkill");
    command.args(["/PID", &pid.to_string(), "/T", "/F"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = command.output();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> Line {
        Line::Text(s.to_string())
    }

    #[test]
    fn init_line() {
        let line = r#"{"type":"system","subtype":"init","session_id":"abc","tools":[]}"#;
        assert_eq!(parse(line), vec![text("session started")]);
    }

    #[test]
    fn assistant_text_splits_lines() {
        let line =
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"one\ntwo"}]}}"#;
        assert_eq!(parse(line), vec![text("one"), text("two")]);
    }

    #[test]
    fn assistant_tool_use() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"cargo test"}}]}}"#;
        assert_eq!(
            parse(line),
            vec![text(r#"→ Bash {"command":"cargo test"}"#)]
        );
    }

    #[test]
    fn tool_use_input_truncated() {
        let long = "x".repeat(200);
        let line = format!(
            r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"Write","input":{{"content":"{long}"}}}}]}}}}"#
        );
        let lines = parse(&line);
        let [Line::Text(out)] = lines.as_slice() else {
            panic!("expected one text line");
        };
        assert_eq!(out.chars().count(), "→ Write ".chars().count() + 80);
    }

    #[test]
    fn tool_error_shown() {
        let line = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"permission denied","is_error":true}]}}"#;
        assert_eq!(parse(line), vec![text("← error permission denied")]);
    }

    #[test]
    fn tool_error_with_block_content() {
        let line = r#"{"type":"user","message":{"content":[{"type":"tool_result","content":[{"type":"text","text":"boom"}],"is_error":true}]}}"#;
        assert_eq!(parse(line), vec![text("← error boom")]);
    }

    #[test]
    fn tool_success_skipped() {
        let line = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"ok"}]}}"#;
        assert!(parse(line).is_empty());
    }

    #[test]
    fn result_line() {
        let line = r#"{"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.01,"num_turns":1,"result":"done"}"#;
        assert_eq!(
            parse(line),
            vec![Line::Done {
                subtype: "success".to_string(),
                cost: Some(0.01),
                turns: Some(1),
            }]
        );
    }

    #[test]
    fn non_json_passes_through() {
        assert_eq!(
            parse("Error: not logged in"),
            vec![text("Error: not logged in")]
        );
    }

    #[test]
    fn unknown_type_skipped() {
        assert!(parse(r#"{"type":"stream_event","event":{}}"#).is_empty());
    }
}
