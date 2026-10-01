//! Readline handles incoming output without losing the draft being edited.
use rustyline_async::{Readline, ReadlineEvent, SharedWriter};
use std::io::{self, IsTerminal, Write};
use tokio::io::{AsyncBufReadExt, BufReader, Lines, Stdin};

use crate::{display, Result};

pub enum Input {
    Terminal(Readline),
    Piped(Lines<BufReader<Stdin>>),
}

impl Input {
    pub async fn next_line(&mut self) -> Result<Option<String>> {
        match self {
            Self::Terminal(readline) => match readline.readline().await? {
                ReadlineEvent::Line(line) => {
                    if !line.is_empty() {
                        readline.add_history_entry(line.clone());
                    }
                    Ok(Some(line))
                }
                ReadlineEvent::Eof | ReadlineEvent::Interrupted => Ok(None),
            },
            Self::Piped(lines) => Ok(lines.next_line().await?),
        }
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        if let Self::Terminal(readline) = self {
            let _ = readline.flush();
        }
        // Readline's Drop restores cooked terminal mode even on errors.
    }
}

#[derive(Clone)]
pub struct Output {
    writer: Option<SharedWriter>,
    colors: bool,
    pub debug: bool,
}

impl Output {
    pub fn plain(debug: bool) -> Self {
        Self {
            writer: None,
            colors: false,
            debug,
        }
    }

    pub fn line(&self, text: &str) -> io::Result<()> {
        match &self.writer {
            Some(writer) => writeln!(writer.clone(), "{text}"),
            None => writeln!(io::stdout().lock(), "{text}"),
        }
    }

    pub fn diagnostic(&self, text: &str) -> io::Result<()> {
        if self.debug {
            self.line(text)?;
        }
        Ok(())
    }

    pub fn peer(&self, name: &str, id: &[u8; 8]) -> String {
        paint(&display(name), peer_color(id), self.colors)
    }

    pub fn own(&self) -> String {
        paint("you", 36, self.colors)
    }

    pub fn message(&self, name: &str, id: &[u8; 8], text: &str) -> String {
        format!("[{}] {}", self.peer(name, id), display(text))
    }
}

fn paint(text: &str, color: u8, enabled: bool) -> String {
    if enabled {
        format!("\x1b[1;{color}m{text}\x1b[0m")
    } else {
        text.to_owned()
    }
}

/// Stable for a peer identity, independent of arrival order or nickname changes.
fn peer_color(id: &[u8; 8]) -> u8 {
    const PALETTE: [u8; 8] = [32, 33, 34, 35, 92, 93, 94, 95];
    PALETTE[(u64::from_be_bytes(*id) % PALETTE.len() as u64) as usize]
}

fn colors_enabled(terminal: bool, no_color: bool, term: &str) -> bool {
    terminal && !no_color && term != "dumb"
}

pub fn open(debug: bool) -> Result<(Input, Output)> {
    let term = std::env::var("TERM").unwrap_or_default();
    let terminal = io::stdin().is_terminal() && io::stdout().is_terminal() && term != "dumb";
    if !terminal {
        return Ok((
            Input::Piped(BufReader::new(tokio::io::stdin()).lines()),
            Output::plain(debug),
        ));
    }
    let colors = colors_enabled(terminal, std::env::var_os("NO_COLOR").is_some(), &term);
    let (mut readline, writer) = Readline::new(paint("you> ", 36, colors))?;
    readline.should_print_line_on(false, false);
    readline.set_max_history(100); // Memory only; never persist chat history to disk.
    Ok((
        Input::Terminal(readline),
        Output {
            writer: Some(writer),
            colors,
            debug,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_are_stable_and_distinct_from_own_color() {
        for value in 0..=255 {
            let id = [value; 8];
            assert_eq!(peer_color(&id), peer_color(&id));
            assert_ne!(peer_color(&id), 36);
        }
        assert_ne!(peer_color(&[0; 8]), peer_color(&[0, 0, 0, 0, 0, 0, 0, 1]));
    }

    #[test]
    fn color_respects_terminal_and_no_color() {
        assert!(colors_enabled(true, false, "xterm-256color"));
        assert!(!colors_enabled(false, false, "xterm-256color"));
        assert!(!colors_enabled(true, true, "xterm-256color"));
        assert!(!colors_enabled(true, false, "dumb"));
    }

    #[test]
    fn message_format_is_safe_and_plain_when_redirected() {
        let output = Output::plain(false);
        assert_eq!(output.message("iphone", &[0; 8], "hello"), "[iphone] hello");
        assert_eq!(output.message("a\x1b", &[0; 8], "hi\r"), "[a\\u{1b}] hi\\r");
        let colored = Output {
            colors: true,
            ..output
        };
        assert_eq!(
            colored.message("iphone", &[0; 8], "hello"),
            "[\x1b[1;32miphone\x1b[0m] hello"
        );
        assert_eq!(colored.own(), "\x1b[1;36myou\x1b[0m");
    }

    /// Run under a PTY: type "draft text" with an incoming message mid-draft.
    #[tokio::test]
    #[ignore = "requires an interactive PTY"]
    async fn incoming_output_preserves_draft() {
        let (mut input, output) = open(false).unwrap();
        assert!(matches!(input, Input::Terminal(_)));
        let incoming = output.clone();
        let print = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            incoming
                .line(&incoming.message("iphone", &[0; 8], "incoming during draft"))
                .unwrap();
        });
        let mut tick = tokio::time::interval(std::time::Duration::from_millis(100));
        let line = loop {
            tokio::select! {
                line = input.next_line() => break line.unwrap(),
                _ = tick.tick() => {} // Mimic BLE events cancelling/re-polling readline.
            }
        };
        assert_eq!(line, Some("draft text".into()));
        print.await.unwrap();
        drop(output);
    }

    #[tokio::test]
    #[ignore = "requires an interactive PTY"]
    async fn exit_event_returns_none() {
        let (mut input, _output) = open(false).unwrap();
        assert!(matches!(input, Input::Terminal(_)));
        assert!(input.next_line().await.unwrap().is_none());
    }
}
