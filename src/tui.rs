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
use ratatui::widgets::{Block, Borders, Paragraph, Tabs};

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
    history: Vec<String>,
    status: String,
    tab_idx: usize,
    input_mode: bool,
    connected: bool,
    should_quit: bool,
    socket: Option<TcpStream>,
}

impl App {
    fn new(host: String, port: String) -> Self {
        Self {
            host,
            port,
            input: String::new(),
            history: vec!["Press 'i' to type, Enter to run command, 'q' to quit.".to_string()],
            status: "Disconnected".to_string(),
            tab_idx: 0,
            input_mode: false,
            connected: false,
            should_quit: false,
            socket: None,
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
                }
                KeyCode::Char(c) => {
                    if !key.modifiers.contains(KeyModifiers::CONTROL) {
                        self.input.push(c);
                    }
                }
                KeyCode::Tab => self.tab_idx = (self.tab_idx + 1) % 3,
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('i') => self.input_mode = true,
            KeyCode::Char('r') => self.connect(),
            KeyCode::Tab => self.tab_idx = (self.tab_idx + 1) % 3,
            _ => {}
        }
    }

    fn execute_input(&mut self) {
        let cmd = self.input.trim().to_string();
        if cmd.is_empty() {
            self.input.clear();
            return;
        }

        if cmd.eq_ignore_ascii_case("clear") {
            self.history.clear();
            self.input.clear();
            return;
        }

        self.history.push(format!("rustis> {}", cmd));

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
                self.history.push(format!("(error) {}", e));
                self.status = format!("Command failed: {}", e);
                self.connected = false;
                self.socket = None;
            }
        }

        while self.history.len() > 500 {
            self.history.remove(0);
        }

        self.input.clear();
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

    fn visible_history(&self, max_rows: usize) -> Vec<String> {
        if self.history.len() <= max_rows {
            return self.history.clone();
        }
        self.history[self.history.len().saturating_sub(max_rows)..].to_vec()
    }
}

fn ui(frame: &mut ratatui::Frame, app: &App) {
    let size = frame.area();

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(size);

    let header = Paragraph::new(Line::from(vec![
        Span::styled("Rustis TUI Client", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(" - connected to "),
        Span::styled(format!("{}:{}", app.host, app.port), Style::default().fg(Color::Cyan)),
    ]))
    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Blue)));
    frame.render_widget(header, sections[0]);

    let tab_titles = ["Terminal", "Commands", "Monitor"]
        .iter()
        .map(|t| Line::from(Span::raw(*t)))
        .collect::<Vec<_>>();
    let tabs = Tabs::new(tab_titles)
        .select(app.tab_idx)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
        .style(Style::default().fg(Color::Gray))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
    frame.render_widget(tabs, sections[1]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
        .split(sections[2]);

    let history_height = body[0].height.saturating_sub(2) as usize;
    let history_text = app.visible_history(history_height).join("\n");
    let history = Paragraph::new(history_text)
        .block(
            Block::default()
                .title("Command History")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(history, body[0]);

    let side_text = match app.tab_idx {
        0 => vec![
            "Quick Commands:",
            "PING",
            "KEYS *",
            "SET mykey hello",
            "GET mykey",
            "INCR counter",
            "LPUSH list item",
            "ZRANGE board 0 -1",
        ],
        1 => vec![
            "Command Tab:",
            "Use this to discover commands.",
            "Try:",
            "AUTH <password>",
            "XADD mystream * field value",
            "XRANGE mystream - +",
            "SUBSCRIBE channel",
        ],
        _ => vec![
            "Monitor Tab:",
            "This panel can show metrics later.",
            "Suggested next additions:",
            "- ops/sec",
            "- memory usage",
            "- connected clients",
        ],
    }
    .join("\n");

    let side = Paragraph::new(side_text)
        .block(
            Block::default()
                .title("Quick Reference")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        )
        .style(Style::default().fg(Color::White));
    frame.render_widget(side, body[1]);

    let mode_label = if app.input_mode { "INPUT" } else { "NORMAL" };
    let input = Paragraph::new(app.input.as_str()).block(
        Block::default()
            .title(format!("{} - press i to type, Esc to stop", mode_label))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(input, sections[3]);

    if app.input_mode {
        let x = sections[3].x + app.input.len() as u16 + 1;
        let y = sections[3].y + 1;
        frame.set_cursor_position((x, y));
    }

    let status = Paragraph::new(app.status.as_str())
        .style(Style::default().fg(if app.connected { Color::Green } else { Color::Red }));
    frame.render_widget(status, sections[4]);
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

fn format_resp_value(value: &RespValue, indent: usize) -> Vec<String> {
    let pad = "  ".repeat(indent);
    match value {
        RespValue::SimpleString(s) => vec![format!("{}{}", pad, s)],
        RespValue::Error(s) => vec![format!("{}(error) {}", pad, s)],
        RespValue::Integer(i) => vec![format!("{}(integer) {}", pad, i)],
        RespValue::BulkString(s) => vec![format!("{}{}", pad, s)],
        RespValue::NullBulk => vec![format!("{}(nil)", pad)],
        RespValue::NullArray => vec![format!("{}(null array)", pad)],
        RespValue::Array(items) => {
            let mut out = Vec::new();
            if items.is_empty() {
                out.push(format!("{}(empty array)", pad));
                return out;
            }

            for (idx, item) in items.iter().enumerate() {
                match item {
                    RespValue::Array(_) => {
                        out.push(format!("{}{})", pad, idx + 1));
                        out.extend(format_resp_value(item, indent + 1));
                    }
                    _ => {
                        let mut lines = format_resp_value(item, 0);
                        if let Some(first) = lines.first_mut() {
                            *first = format!("{}{}) {}", pad, idx + 1, first);
                        }
                        out.extend(lines);
                    }
                }
            }
            out
        }
    }
}
