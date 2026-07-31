use anyhow::Result;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::mpsc,
};

use super::LogEvent;

pub(super) async fn read<R>(
    name: &str,
    stream: R,
    events: Option<mpsc::UnboundedSender<LogEvent>>,
) -> Result<String>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut lines = BufReader::new(stream).lines();
    let mut output = String::new();
    while let Some(line) = lines.next_line().await? {
        output.push_str(&line);
        output.push('\n');
        if let Some(sender) = &events {
            sender
                .send(LogEvent {
                    stream: name.to_owned(),
                    text: line,
                    at: chrono::Utc::now().to_rfc3339(),
                })
                .ok();
        }
    }
    Ok(output)
}
