use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::prelude::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

#[derive(Clone, Debug)]
enum RespValue {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(String),
    Array(Vec<RespValue>),
    NullBulk,
    NullArray,
}

const COMMAND_CATALOG: &[(&str, &str)] = &[
    ("PING", "PING"),
    ("ECHO", "ECHO <message>"),
    ("SET", "SET <key> <value>"),
    ("GET", "GET <key>"),
    ("INCR", "INCR <key>"),
    ("LPUSH", "LPUSH <list> <item...>"),
    ("RPUSH", "RPUSH <list> <item...>"),
    ("LLEN", "LLEN <list>"),
    ("LRANGE", "LRANGE <list> <start> <stop>"),
    ("LPOP", "LPOP <list>"),
    ("TYPE", "TYPE <key>"),
    ("XADD", "XADD <stream> * <field> <value>"),
    ("XRANGE", "XRANGE <stream> <start> <end>"),
    ("XREAD", "XREAD [COUNT n] STREAMS <stream> <id>"),
    ("MULTI", "MULTI"),
    ("EXEC", "EXEC"),
    ("DISCARD", "DISCARD"),
    ("INFO", "INFO"),
    ("REPLCONF", "REPLCONF <arg> <value>"),
    ("PSYNC", "PSYNC <replicationid> <offset>"),
    ("CONFIG", "CONFIG GET <param>"),
    ("KEYS", "KEYS <pattern>"),
    ("SUBSCRIBE", "SUBSCRIBE <channel...>"),
    ("UNSUBSCRIBE", "UNSUBSCRIBE [channel...]"),
    ("PUBLISH", "PUBLISH <channel> <message>"),
    ("ZADD", "ZADD <key> <score> <member> [score member...]"),
    ("ZRANK", "ZRANK <key> <member>"),
    ("ZRANGE", "ZRANGE <key> <start> <stop>"),
    ("ZCARD", "ZCARD <key>"),
    ("ZSCORE", "ZSCORE <key> <member>"),
    ("ZREM", "ZREM <key> <member...>"),
    ("GEOADD", "GEOADD <key> <lon> <lat> <member> [lon lat member...]"),
    ("GEOPOS", "GEOPOS <key> <member...>"),
    ("GEODIST", "GEODIST <key> <member1> <member2> [unit]"),
    ("GEOSEARCH", "GEOSEARCH <key> ..."),
    ("ACL", "ACL LIST"),
    ("AUTH", "AUTH <password>"),
];

#[derive(Clone, Copy)]
struct Theme {
    name: &'static str,
    header_border: Color,
    header_title: Color,
    header_host: Color,
    history_border: Color,
    history_text: Color,
    side_border: Color,
    side_text: Color,
    input_border: Color,
    input_text: Color,
    status_connected: Color,
    status_disconnected: Color,
    cmd_text: Color,
    ok_text: Color,
    err_text: Color,
    int_text: Color,
    nil_text: Color,
    muted_text: Color,
}

const THEMES: [Theme; 2] = [
    Theme {
        name: "Neon",
        header_border: Color::Blue,
        header_title: Color::Green,
        header_host: Color::Cyan,
        history_border: Color::Blue,
        history_text: Color::Cyan,
        side_border: Color::Green,
        side_text: Color::White,
        input_border: Color::Yellow,
        input_text: Color::White,
        status_connected: Color::Green,
        status_disconnected: Color::Red,
        cmd_text: Color::LightCyan,
        ok_text: Color::Green,
        err_text: Color::Red,
        int_text: Color::Yellow,
        nil_text: Color::DarkGray,
        muted_text: Color::DarkGray,
    },
    Theme {
        name: "Amber",
        header_border: Color::LightYellow,
        header_title: Color::Yellow,
        header_host: Color::LightBlue,
        history_border: Color::Yellow,
        history_text: Color::LightYellow,
        side_border: Color::LightMagenta,
        side_text: Color::White,
        input_border: Color::LightRed,
        input_text: Color::White,
        status_connected: Color::LightGreen,
        status_disconnected: Color::LightRed,
        cmd_text: Color::LightBlue,
        ok_text: Color::LightGreen,
        err_text: Color::LightRed,
        int_text: Color::LightYellow,
        nil_text: Color::Gray,
        muted_text: Color::Gray,
    },
];

#[derive(Clone, Copy)]
enum HistoryKind {
    Info,
    Command,
    Ok,
    Error,
    Integer,
    Nil,
    Value,
    Muted,
}

#[derive(Clone)]
struct HistoryLine {
    text: String,
    kind: HistoryKind,
}

pub fn run(host: &str, port: &str) -> Result<()> {
    let mut app = App::new(host.to_string(), port.to_string());
    app.connect();

    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("failed to enter alternate screen")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to create terminal")?;

    let mut run_result = Ok(());

    while !app.should_quit {
        if let Err(e) = terminal.draw(|f| ui(f, &app)) {
            run_result = Err(e).context("failed to draw frame");
            break;
        }

        match event::poll(Duration::from_millis(50)) {
            Ok(true) => match event::read() {
                Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => app.handle_key(key),
                Ok(_) => {}
                Err(e) => {
                    run_result = Err(e).context("failed to read terminal event");
                    break;
                }
            },
            Ok(false) => {}
            Err(e) => {
                run_result = Err(e).context("failed to poll terminal event");
                break;
            }
        }
    }

    disable_raw_mode().context("failed to disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .context("failed to leave alternate screen")?;
    terminal.show_cursor().context("failed to show cursor")?;

    run_result
}

struct App {
    host: String,
    port: String,
    input: String,
    history: Vec<HistoryLine>,
    status: String,
    input_mode: bool,
    connected: bool,
    should_quit: bool,
    socket: Option<TcpStream>,
    command_history: Vec<String>,
    history_cursor: Option<usize>,
    history_draft: String,
    completion_prefix: String,
    completion_candidates: Vec<String>,
    completion_idx: usize,
    theme_idx: usize,
}

impl App {
    fn new(host: String, port: String) -> Self {
        Self {
            host,
            port,
            input: String::new(),
            history: vec![HistoryLine {
                text: "Press 'i' to type, Enter to run command, 'q' to quit.".to_string(),
                kind: HistoryKind::Info,
            }],
            status: "Disconnected".to_string(),
            input_mode: false,
            connected: false,
            should_quit: false,
            socket: None,
            command_history: Vec::new(),
            history_cursor: None,
            history_draft: String::new(),
            completion_prefix: String::new(),
            completion_candidates: Vec::new(),
            completion_idx: 0,
            theme_idx: 0,
        }
    }

    fn connect(&mut self) {
        let addr = format!("{}:{}", self.host, self.port);
        match TcpStream::connect(&addr) {
            Ok(stream) => {
                if stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .is_err()
                    || stream.set_write_timeout(Some(Duration::from_secs(2))).is_err()
                {
                    self.status = "Connected, but failed to configure socket timeouts".to_string();
                } else {
                    self.status = format!("Connected to {}", addr);
                }
                let _ = stream.set_nodelay(true);
                self.connected = true;
                self.socket = Some(stream);
            }
            Err(e) => {
                self.status = format!("Connection failed: {}", e);
                self.connected = false;
                self.socket = None;
            }
        }
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        if self.input_mode {
            match key.code {
                KeyCode::Esc => self.input_mode = false,
                KeyCode::Enter => self.execute_input(),
                KeyCode::Backspace => {
                    self.input.pop();
                    self.reset_edit_state();
                }
                KeyCode::Char(c) => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) && (c == 't' || c == 'T') {
                        self.cycle_theme();
                    } else if !key.modifiers.contains(KeyModifiers::CONTROL) {
                        self.input.push(c);
                        self.reset_edit_state();
                    }
                }
                KeyCode::Up => self.recall_previous_command(),
                KeyCode::Down => self.recall_next_command(),
                KeyCode::Tab => self.autocomplete_command(),
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('i') => self.input_mode = true,
            KeyCode::Char('r') => self.connect(),
            KeyCode::Char('t') => self.cycle_theme(),
            KeyCode::F(2) => self.cycle_theme(),
            _ => {}
        }
    }

    fn execute_input(&mut self) {
        let cmd = self.input.trim().to_string();
        if cmd.is_empty() {
            self.input.clear();
            self.clear_navigation_state();
            return;
        }

        if cmd.eq_ignore_ascii_case("clear") {
            self.history.clear();
            self.input.clear();
            self.clear_navigation_state();
            return;
        }

        self.command_history.push(cmd.clone());
        self.history.push(HistoryLine {
            text: format!("rustis> {}", cmd),
            kind: HistoryKind::Command,
        });

        if !self.connected || self.socket.is_none() {
            self.connect();
        }

        let result = self.send_command(&cmd);
        match result {
            Ok(value) => {
                for line in format_resp_value(&value, 0) {
                    self.history.push(line);
                }
                self.status = format!("Executed: {}", cmd.split_whitespace().next().unwrap_or(""));
            }
            Err(e) => {
                self.history.push(HistoryLine {
                    text: format!("(error) {}", e),
                    kind: HistoryKind::Error,
                });
                self.status = format!("Command failed: {}", e);
                self.connected = false;
                self.socket = None;
            }
        }

        while self.history.len() > 500 {
            self.history.remove(0);
        }

        self.input.clear();
        self.clear_navigation_state();
    }

    fn send_command(&mut self, input: &str) -> Result<RespValue> {
        let payload = serialize_command(input)?;
        let socket = self
            .socket
            .as_mut()
            .context("not connected to server (press 'r' to retry)")?;

        socket
            .write_all(payload.as_bytes())
            .context("failed to write command to server")?;
        socket.flush().context("failed to flush command")?;

        read_resp_value(socket).context("failed to read server response")
    }

    fn visible_history(&self, max_rows: usize) -> Vec<HistoryLine> {
        if self.history.len() <= max_rows {
            return self.history.clone();
        }
        self.history[self.history.len().saturating_sub(max_rows)..].to_vec()
    }

    fn theme(&self) -> &'static Theme {
        &THEMES[self.theme_idx]
    }

    fn cycle_theme(&mut self) {
        self.theme_idx = (self.theme_idx + 1) % THEMES.len();
        self.status = format!("Theme: {}", self.theme().name);
    }

    fn reset_edit_state(&mut self) {
        self.history_cursor = None;
        self.history_draft.clear();
        self.completion_prefix.clear();
        self.completion_candidates.clear();
        self.completion_idx = 0;
    }

    fn clear_navigation_state(&mut self) {
        self.history_cursor = None;
        self.history_draft.clear();
        self.completion_prefix.clear();
        self.completion_candidates.clear();
        self.completion_idx = 0;
    }

    fn recall_previous_command(&mut self) {
        if self.command_history.is_empty() {
            return;
        }

        match self.history_cursor {
            Some(idx) if idx > 0 => self.history_cursor = Some(idx - 1),
            Some(_) => {}
            None => {
                self.history_draft = self.input.clone();
                self.history_cursor = Some(self.command_history.len() - 1);
            }
        }

        if let Some(idx) = self.history_cursor {
            self.input = self.command_history[idx].clone();
        }
        self.completion_prefix.clear();
        self.completion_candidates.clear();
        self.completion_idx = 0;
    }

    fn recall_next_command(&mut self) {
        let Some(idx) = self.history_cursor else {
            return;
        };

        if idx + 1 < self.command_history.len() {
            self.history_cursor = Some(idx + 1);
            self.input = self.command_history[idx + 1].clone();
        } else {
            self.history_cursor = None;
            self.input = std::mem::take(&mut self.history_draft);
        }

        self.completion_prefix.clear();
        self.completion_candidates.clear();
        self.completion_idx = 0;
    }

    fn autocomplete_command(&mut self) {
        let leading = self.input.len() - self.input.trim_start().len();
        let prefix_source = self.input.trim_start();

        if prefix_source.contains(char::is_whitespace) {
            self.status = "Autocomplete applies to command name (first token)".to_string();
            return;
        }

        let prefix = prefix_source.to_ascii_uppercase();
        if self.completion_prefix != prefix {
            self.completion_prefix = prefix.clone();
            self.completion_candidates = COMMAND_CATALOG
                .iter()
                .map(|(name, _)| (*name).to_string())
                .filter(|name| name.starts_with(&prefix))
                .collect();
            self.completion_idx = 0;
        } else if !self.completion_candidates.is_empty() {
            self.completion_idx = (self.completion_idx + 1) % self.completion_candidates.len();
        }

        if self.completion_candidates.is_empty() {
            self.status = format!("No command match for '{}'", prefix);
            return;
        }

        let completed = &self.completion_candidates[self.completion_idx];
        let left_pad = &self.input[..leading];
        self.input = format!("{}{} ", left_pad, completed);
        self.history_cursor = None;
        self.history_draft.clear();
        self.status = format!(
            "Autocomplete {}/{}: {}",
            self.completion_idx + 1,
            self.completion_candidates.len(),
            completed
        );
    }
}

fn ui(frame: &mut ratatui::Frame, app: &App) {
    let theme = app.theme();
    let size = frame.area();

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(size);

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            format!("Rustis TUI Client [{}]", theme.name),
            Style::default()
                .fg(theme.header_title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" - connected to "),
        Span::styled(
            format!("{}:{}", app.host, app.port),
            Style::default().fg(theme.header_host),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.header_border)),
    );
    frame.render_widget(header, sections[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
        .split(sections[1]);

    let history_height = body[0].height.saturating_sub(2) as usize;
    let history_lines = app
        .visible_history(history_height)
        .into_iter()
        .map(|line| {
            Line::from(Span::styled(
                line.text,
                Style::default().fg(history_color_for_kind(theme, line.kind)),
            ))
        })
        .collect::<Vec<_>>();
    let history = Paragraph::new(history_lines)
        .block(
            Block::default()
                .title("Command History")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.history_border)),
        )
        .style(Style::default().fg(theme.history_text));
    frame.render_widget(history, body[0]);

    let side_text = vec![
        "Quick Commands:",
        "PING",
        "KEYS *",
        "SET mykey hello",
        "GET mykey",
        "INCR counter",
        "LPUSH list item",
        "ZRANGE board 0 -1",
    ]
    .join("\n");

    let side = Paragraph::new(side_text)
        .block(
            Block::default()
                .title("Quick Reference")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.side_border)),
        )
        .style(Style::default().fg(theme.side_text));
    frame.render_widget(side, body[1]);

    let mode_label = if app.input_mode { "INPUT" } else { "NORMAL" };
    let input = Paragraph::new(app.input.as_str()).block(
        Block::default()
            .title(format!(
                "{} - press i to type, Esc to stop, Ctrl+T/F2 to change theme",
                mode_label
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.input_border)),
    );
    let input = input.style(Style::default().fg(theme.input_text));
    frame.render_widget(input, sections[2]);

    if app.input_mode {
        let x = sections[2].x + app.input.len() as u16 + 1;
        let y = sections[2].y + 1;
        frame.set_cursor_position((x, y));
    }

    let mut status_line = app.status.clone();
    if app.input_mode {
        if let Some(hint) = command_hint(&app.input) {
            status_line = format!("{} | Hint: {}", status_line, hint);
        }
    }

    let status = Paragraph::new(status_line)
        .style(Style::default().fg(if app.connected {
            theme.status_connected
        } else {
            theme.status_disconnected
        }));
    frame.render_widget(status, sections[3]);
}

fn history_color_for_kind(theme: &Theme, kind: HistoryKind) -> Color {
    match kind {
        HistoryKind::Info => theme.history_text,
        HistoryKind::Command => theme.cmd_text,
        HistoryKind::Ok => theme.ok_text,
        HistoryKind::Error => theme.err_text,
        HistoryKind::Integer => theme.int_text,
        HistoryKind::Nil => theme.nil_text,
        HistoryKind::Value => theme.history_text,
        HistoryKind::Muted => theme.muted_text,
    }
}

fn command_hint(input: &str) -> Option<&'static str> {
    let token = tokenize(input).into_iter().next()?;
    let command = token.to_ascii_uppercase();
    COMMAND_CATALOG
        .iter()
        .find(|(name, _)| *name == command)
        .map(|(_, hint)| *hint)
}

fn serialize_command(input: &str) -> Result<String> {
    let parts = tokenize(input);
    if parts.is_empty() {
        return Err(anyhow::anyhow!("empty command"));
    }

    let mut payload = format!("*{}\r\n", parts.len());
    for part in parts {
        payload.push_str(&format!("${}\r\n{}\r\n", part.len(), part));
    }

    Ok(payload)
}

fn tokenize(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;

    for ch in input.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => current.push(ch),
            None if ch == '\'' || ch == '"' => quote = Some(ch),
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            None => current.push(ch),
        }
    }

    if !current.is_empty() {
        out.push(current);
    }

    out
}

fn read_resp_value(stream: &mut TcpStream) -> Result<RespValue> {
    let mut prefix = [0u8; 1];
    stream.read_exact(&mut prefix)?;

    match prefix[0] {
        b'+' => Ok(RespValue::SimpleString(read_crlf_line(stream)?)),
        b'-' => Ok(RespValue::Error(read_crlf_line(stream)?)),
        b':' => {
            let value = read_crlf_line(stream)?.parse::<i64>()?;
            Ok(RespValue::Integer(value))
        }
        b'$' => {
            let len = read_crlf_line(stream)?.parse::<i64>()?;
            if len < 0 {
                return Ok(RespValue::NullBulk);
            }

            let mut buf = vec![0u8; len as usize];
            stream.read_exact(&mut buf)?;
            expect_crlf(stream)?;
            Ok(RespValue::BulkString(String::from_utf8(buf)?))
        }
        b'*' => {
            let len = read_crlf_line(stream)?.parse::<i64>()?;
            if len < 0 {
                return Ok(RespValue::NullArray);
            }

            let mut items = Vec::with_capacity(len as usize);
            for _ in 0..len {
                items.push(read_resp_value(stream)?);
            }
            Ok(RespValue::Array(items))
        }
        other => Err(anyhow::anyhow!("unknown RESP prefix byte: {}", other)),
    }
}

fn read_crlf_line(stream: &mut TcpStream) -> Result<String> {
    let mut bytes = Vec::new();
    loop {
        let mut b = [0u8; 1];
        stream.read_exact(&mut b)?;
        bytes.push(b[0]);

        let len = bytes.len();
        if len >= 2 && bytes[len - 2] == b'\r' && bytes[len - 1] == b'\n' {
            bytes.truncate(len - 2);
            return Ok(String::from_utf8(bytes)?);
        }
    }
}

fn expect_crlf(stream: &mut TcpStream) -> Result<()> {
    let mut crlf = [0u8; 2];
    stream.read_exact(&mut crlf)?;
    if crlf == [b'\r', b'\n'] {
        Ok(())
    } else {
        Err(anyhow::anyhow!("invalid RESP line terminator"))
    }
}

fn format_resp_value(value: &RespValue, indent: usize) -> Vec<HistoryLine> {
    let pad = "  ".repeat(indent);
    match value {
        RespValue::SimpleString(s) => vec![HistoryLine {
            text: format!("{}{}", pad, s),
            kind: HistoryKind::Ok,
        }],
        RespValue::Error(s) => vec![HistoryLine {
            text: format!("{}(error) {}", pad, s),
            kind: HistoryKind::Error,
        }],
        RespValue::Integer(i) => vec![HistoryLine {
            text: format!("{}(integer) {}", pad, i),
            kind: HistoryKind::Integer,
        }],
        RespValue::BulkString(s) => vec![HistoryLine {
            text: format!("{}{}", pad, s),
            kind: HistoryKind::Value,
        }],
        RespValue::NullBulk => vec![HistoryLine {
            text: format!("{}(nil)", pad),
            kind: HistoryKind::Nil,
        }],
        RespValue::NullArray => vec![HistoryLine {
            text: format!("{}(null array)", pad),
            kind: HistoryKind::Nil,
        }],
        RespValue::Array(items) => {
            let mut out = Vec::new();
            if items.is_empty() {
                out.push(HistoryLine {
                    text: format!("{}(empty array)", pad),
                    kind: HistoryKind::Nil,
                });
                return out;
            }

            for (idx, item) in items.iter().enumerate() {
                match item {
                    RespValue::Array(_) => {
                        out.push(HistoryLine {
                            text: format!("{}{})", pad, idx + 1),
                            kind: HistoryKind::Muted,
                        });
                        out.extend(format_resp_value(item, indent + 1));
                    }
                    _ => {
                        let mut lines = format_resp_value(item, 0);
                        if let Some(first) = lines.first_mut() {
                            first.text = format!("{}{}) {}", pad, idx + 1, first.text);
                        }
                        out.extend(lines);
                    }
                }
            }
            out
        }
    }
}
