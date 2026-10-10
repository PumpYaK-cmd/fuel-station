use std::collections::VecDeque;
use std::path::PathBuf;

use iced::Task;
use iced::futures::SinkExt;
use iced::futures::channel::mpsc::Sender;
use iced::task::Handle;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, BufReader};

use super::Message;
use crate::launch::{self, Line};

const LOG_CAP: usize = 300;

#[derive(Debug, Clone)]
pub enum RunEvent {
    Started(Option<u32>),
    Out(Line),
    Ended(Option<i32>),
    FailedToStart(String),
}

pub enum Status {
    Running,
    Exited {
        code: Option<i32>,
        subtype: Option<String>,
        cost: Option<f64>,
        turns: Option<i64>,
    },
    Stopped,
    Failed(String),
}

pub struct Run {
    pub id: u64,
    pid: Option<u32>,
    pub lines: VecDeque<String>,
    pub status: Status,
    done: Option<Line>,
    handle: Handle,
}

impl Run {
    pub fn is_running(&self) -> bool {
        matches!(self.status, Status::Running)
    }

    pub fn apply(&mut self, event: RunEvent) -> Option<String> {
        match event {
            RunEvent::Started(pid) => self.pid = pid,
            RunEvent::Out(Line::Text(line)) => {
                if self.lines.len() == LOG_CAP {
                    self.lines.pop_front();
                }
                self.lines.push_back(line);
            }
            RunEvent::Out(done) => self.done = Some(done),
            RunEvent::Ended(code) if self.is_running() => return Some(self.finish(code)),
            RunEvent::Ended(_) => {}
            RunEvent::FailedToStart(e) => {
                let report = format!("Truck failed to start: {e}");
                self.status = Status::Failed(e);
                return Some(report);
            }
        }
        None
    }

    fn finish(&mut self, code: Option<i32>) -> String {
        let (subtype, cost, turns) = match self.done.take() {
            Some(Line::Done {
                subtype,
                cost,
                turns,
            }) => (Some(subtype), cost, turns),
            _ => (None, None, None),
        };
        let report = match (&subtype, cost, code) {
            (Some(s), Some(c), _) => format!("Truck run exited: {s}, ${c:.2}"),
            (Some(s), None, _) => format!("Truck run exited: {s}"),
            (None, _, Some(code)) => format!("Truck run exited: code {code}"),
            (None, _, None) => "Truck run exited".to_string(),
        };
        self.status = Status::Exited {
            code,
            subtype,
            cost,
            turns,
        };
        report
    }

    pub fn stop(&mut self) {
        if let Some(pid) = self.pid {
            launch::kill_tree(pid);
        }
        self.handle.abort();
        self.status = Status::Stopped;
    }

    pub fn status_text(&self) -> String {
        match &self.status {
            Status::Running => "running".to_string(),
            Status::Exited {
                code,
                subtype,
                cost,
                turns,
            } => {
                let mut parts = vec![match code {
                    Some(code) => format!("exited {code}"),
                    None => "exited".to_string(),
                }];
                parts.extend(subtype.clone());
                parts.extend(cost.map(|c| format!("${c:.2}")));
                parts.extend(turns.map(|t| format!("{t} turns")));
                parts.join(" · ")
            }
            Status::Stopped => "stopped".to_string(),
            Status::Failed(e) => format!("failed to start: {e}"),
        }
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        if self.is_running() {
            self.stop();
        }
    }
}

pub fn start(dir: PathBuf, key: String, id: u64) -> (Task<Message>, Run) {
    let card_key = key.clone();
    let stream = iced::stream::channel(100, async move |mut sender: Sender<RunEvent>| {
        let mut child = match launch::command(&dir, &card_key).spawn() {
            Ok(child) => child,
            Err(e) => {
                let _ = sender.send(RunEvent::FailedToStart(e.to_string())).await;
                return;
            }
        };
        let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
            let _ = sender
                .send(RunEvent::FailedToStart("no output pipe".to_string()))
                .await;
            return;
        };
        let _ = sender.send(RunEvent::Started(child.id())).await;
        let mut out = BufReader::new(stdout);
        let mut err = BufReader::new(stderr);
        let (mut out_open, mut err_open) = (true, true);
        while out_open || err_open {
            let (from_out, line) = tokio::select! {
                line = read_line(&mut out), if out_open => (true, line),
                line = read_line(&mut err), if err_open => (false, line),
            };
            match line {
                Some(line) => {
                    for parsed in launch::parse(&line) {
                        let _ = sender.send(RunEvent::Out(parsed)).await;
                    }
                }
                _ if from_out => out_open = false,
                _ => err_open = false,
            }
        }
        let code = child.wait().await.ok().and_then(|s| s.code());
        let _ = sender.send(RunEvent::Ended(code)).await;
    });
    let (task, handle) = Task::run(stream, move |event| {
        Message::RunEvent(key.clone(), id, event)
    })
    .abortable();
    let run = Run {
        id,
        pid: None,
        lines: VecDeque::new(),
        status: Status::Running,
        done: None,
        handle,
    };
    (task, run)
}

async fn read_line(reader: &mut (impl AsyncBufRead + Unpin)) -> Option<String> {
    let mut buf = Vec::new();
    match reader.read_until(b'\n', &mut buf).await {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(String::from_utf8_lossy(&buf).trim_end().to_string()),
    }
}
