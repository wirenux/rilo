use crossterm::{
    self,
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode, size},
};
use std::{
    env::{self},
    time::Instant,
};
use std::{fs, io};
use std::{
    io::{Write, stdout},
    time::Duration,
};

const RILO_VERSION: &str = env!("CARGO_PKG_VERSION");
const TAB_STOP: usize = 8;
const QUIT_TIMES: u8 = 1;

struct EditorConfig {
    terminal_cols: u16,
    terminal_rows: u16,
    cursor_x: u16,
    cursor_y: u16,
    rows: Vec<EditorRow>,
    row_offset: u16,
    col_offset: u16,
    render_x: u16,
    filename: String,
    status_message: String,
    status_message_time: Option<Instant>,
    dirty: bool,
    search_query: String,
    last_match: Option<usize>,
    search_direction: i8,
}

struct EditorRow {
    chars: String,
    render: String,
}

impl EditorRow {
    fn new(chars: String) -> Self {
        let mut row = Self {
            chars,
            render: String::new(),
        };
        row.update_render();
        row
    }

    fn cx_to_rx(&self, cursor_x: usize) -> usize {
        let mut render_x: usize = 0;

        for (i, ch) in self.chars.chars().enumerate() {
            if i >= cursor_x {
                break;
            }
            if ch == '\t' {
                render_x += (TAB_STOP - 1) - (render_x % TAB_STOP);
            }
            render_x += 1;
        }

        render_x
    }

    fn rx_to_cx(&self, render_x: usize) -> usize {
        let mut cursor_x = 0;

        for (cx, ch) in self.chars.chars().enumerate() {
            if ch == '\t' {
                cursor_x += (TAB_STOP - 1) - (cursor_x % TAB_STOP);
            }

            cursor_x += 1;

            if cursor_x > render_x {
                return cx;
            }
        }

        self.chars.chars().count()
    }

    fn update_render(&mut self) {
        let tab_count = self.chars.chars().filter(|&c| c == '\t').count();
        self.render.clear();
        self.render.reserve(self.chars.len() + tab_count * 7);

        let mut idx = 0;
        for ch in self.chars.chars() {
            if ch == '\t' {
                self.render.push(' ');
                idx += 1;
                while idx % TAB_STOP != 0 {
                    self.render.push(' ');
                    idx += 1;
                }
            } else {
                self.render.push(ch);
                idx += 1;
            }
        }
    }

    fn insert_char(&mut self, at: usize, c: char) {
        let byte_idx = self
            .chars
            .char_indices()
            .nth(at)
            .map_or(self.chars.len(), |(idx, _)| idx);

        self.chars.insert(byte_idx, c);
        self.update_render();
    }

    fn del_char(&mut self, at: usize) {
        if let Some((byte_idx, _)) = self.chars.char_indices().nth(at) {
            self.chars.remove(byte_idx);
            self.update_render();
        }
    }
}

impl EditorConfig {
    fn new() -> io::Result<Self> {
        let (cols, mut rows) = size()?;
        rows = rows.saturating_sub(2);

        Ok(Self {
            terminal_cols: cols,
            terminal_rows: rows,
            cursor_y: 0,
            cursor_x: 0,
            rows: Vec::new(),
            row_offset: 0,
            col_offset: 0,
            render_x: 0,
            filename: String::new(),
            status_message: String::new(),
            status_message_time: None,
            dirty: false,
            search_query: String::new(),
            last_match: None,
            search_direction: 1,
        })
    }

    fn open(&mut self, path: &str) -> io::Result<()> {
        self.filename = path.to_string();

        let content = fs::read_to_string(path)?;
        self.rows = content
            .lines()
            .map(|line| EditorRow::new(line.to_string()))
            .collect();
        self.dirty = false;
        Ok(())
    }

    fn move_cursor(&mut self, key: KeyCode) {
        let row = self.rows.get(self.cursor_y as usize);
        match key {
            KeyCode::Char('h') | KeyCode::Left => {
                if self.cursor_x > 0 {
                    self.cursor_x -= 1;
                } else if self.cursor_y > 0 {
                    self.cursor_y -= 1;
                    let prev_row_len = self
                        .rows
                        .get(self.cursor_y as usize)
                        .map_or(0, |r| r.chars.chars().count() as u16);
                    self.cursor_x = prev_row_len;
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if (self.cursor_y as usize) < self.rows.len() {
                    self.cursor_y += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.cursor_y = self.cursor_y.saturating_sub(1);
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if let Some(r) = row {
                    let row_len = r.chars.chars().count() as u16;
                    if self.cursor_x < row_len {
                        self.cursor_x += 1;
                    } else if self.cursor_x == row_len
                        && (self.cursor_y as usize) + 1 < self.rows.len()
                    {
                        self.cursor_y += 1;
                        self.cursor_x = 0;
                    }
                }
            }

            _ => {}
        }
        let row_len = self
            .rows
            .get(self.cursor_y as usize)
            .map_or(0, |r| r.chars.chars().count() as u16);
        if self.cursor_x > row_len {
            self.cursor_x = row_len;
        }
    }

    fn scroll(&mut self) {
        self.render_x = 0;
        if (self.cursor_y as usize) < self.rows.len() {
            self.render_x =
                self.rows[self.cursor_y as usize].cx_to_rx(self.cursor_x as usize) as u16;
        }

        if self.cursor_y < self.row_offset {
            self.row_offset = self.cursor_y;
        }
        if self.cursor_y >= self.row_offset + self.terminal_rows {
            self.row_offset = self.cursor_y - self.terminal_rows + 1;
        }
        if self.render_x < self.col_offset {
            self.col_offset = self.render_x;
        }
        if self.render_x >= self.col_offset + self.terminal_cols {
            self.col_offset = self.render_x - self.terminal_cols + 1;
        }
    }

    fn set_status_message(&mut self, message: &str) {
        self.status_message = message.to_string();
        self.status_message_time = Some(Instant::now());
    }

    fn insert_char(&mut self, c: char) {
        if (self.cursor_y as usize) == self.rows.len() {
            self.rows.push(EditorRow::new(String::new()));
        }

        self.rows[self.cursor_y as usize].insert_char(self.cursor_x as usize, c);
        self.dirty = true;
        self.cursor_x += 1;
    }

    fn del_char(&mut self) {
        let cx = self.cursor_x as usize;
        let cy = self.cursor_y as usize;

        if cy == self.rows.len() || (cx == 0 && cy == 0) {
            return;
        }

        if cx > 0 {
            self.rows[cy].del_char(cx - 1);
            self.cursor_x -= 1;
        } else {
            let current_row = self.rows.remove(cy);
            let prev_row = &mut self.rows[cy - 1];

            self.cursor_x = prev_row.chars.chars().count() as u16;

            prev_row.chars.push_str(&current_row.chars);
            prev_row.update_render();

            self.cursor_y -= 1;
        }

        self.dirty = true;
    }

    fn insert_newline(&mut self) {
        let cx = self.cursor_x as usize;
        let cy = self.cursor_y as usize;

        if cx == 0 {
            // if cursor is at the start of a line (or the bottom ~)
            self.rows.insert(cy, EditorRow::new(String::new()));
        } else {
            // if cursor at the middle of a line
            let byte_idx = self.rows[cy]
                .chars
                .char_indices()
                .nth(cx)
                .map_or(self.rows[cy].chars.len(), |(idx, _)| idx);

            let right_half = self.rows[cy].chars.split_off(byte_idx);
            self.rows[cy].update_render();

            self.rows.insert(cy + 1, EditorRow::new(right_half));
        }

        self.cursor_y += 1;
        self.cursor_x = 0;
        self.dirty = true;
    }

    fn rows_to_string(&self) -> String {
        let total_len: usize = self.rows.iter().map(|r| r.chars.len() + 1).sum();

        let mut buf = String::with_capacity(total_len);
        for row in &self.rows {
            buf.push_str(&row.chars);
            buf.push('\n');
        }

        buf
    }

    fn save(&mut self) -> io::Result<()> {
        if self.filename.is_empty() {
            match self.prompt("Save as: {}")? {
                Some(name) => self.filename = name,
                None => {
                    self.set_status_message("Warning: Save aborted");
                    return Ok(());
                }
            }
        }

        let content = self.rows_to_string();
        let len = content.len();

        match fs::write(&self.filename, content) {
            Ok(()) => {
                self.set_status_message(&format!("{} bytes written to disk", len));
                self.dirty = false;
                Ok(())
            }
            Err(e) => {
                self.set_status_message(&format!("Error: Can't save ! {}", e));
                Err(e)
            }
        }
    }

    fn prompt(&mut self, prompt: &str) -> io::Result<Option<String>> {
        let mut buf = String::new();

        loop {
            let msg = prompt.replacen("{}", &buf, 1);
            self.set_status_message(&msg);

            let message_row = self.terminal_rows + 1;
            let prompt_col = (msg.chars().count() as u16).min(self.terminal_cols.saturating_sub(1));

            refresh_screen(self, Some((prompt_col, message_row)))?;

            if let Event::Key(KeyEvent {
                code,
                modifiers,
                kind: KeyEventKind::Press,
                ..
            }) = event::read()?
            {
                match code {
                    KeyCode::Enter => {
                        if !buf.is_empty() {
                            self.set_status_message("");
                            return Ok(Some(buf));
                        }
                    }
                    KeyCode::Esc => {
                        self.set_status_message("");
                        return Ok(None);
                    }
                    KeyCode::Backspace | KeyCode::Delete => {
                        buf.pop();
                    }
                    KeyCode::Char(c)
                        if !modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                            && !c.is_control() =>
                    {
                        buf.push(c);
                    }
                    _ => {}
                }
            }
        }
    }

    fn find(&mut self) -> io::Result<()> {
        let query = match self.prompt("Search: {}")? {
            Some(query) => query,
            None => return Ok(()),
        };

        self.search_query = query;
        self.last_match = None;
        self.find_next(1);

        Ok(())
    }

    fn find_next(&mut self, direction: i8) {
        if self.search_query.is_empty() || self.rows.is_empty() {
            return;
        }

        let start = self.last_match.unwrap_or(0);

        for step in 1..=self.rows.len() {
            let i = if direction == 1 {
                (start + step) % self.rows.len()
            } else {
                (start + self.rows.len() - (step % self.rows.len())) % self.rows.len()
            };

            if let Some(match_index) = &self.rows[i].render.find(&self.search_query) {
                self.cursor_y = i as u16;
                self.cursor_x = self.rows[i].rx_to_cx(*match_index) as u16;

                self.last_match = Some(i);
                self.search_direction = direction;

                return;
            }
        }

        self.set_status_message("Warning: No match found");
    }
}

fn clear_screen() -> io::Result<()> {
    execute!(io::stdout(), Clear(ClearType::All), MoveTo(0, 0)) // clear screen and put cursor on the top left corner
}

fn draw_rows(out: &mut impl Write, config: &EditorConfig) -> io::Result<()> {
    for y in 0..config.terminal_rows {
        let file_row = (y as usize) + (config.row_offset as usize);
        if file_row >= config.rows.len() {
            if config.rows.is_empty() && y == config.terminal_rows / 3 {
                let mut welcome_text = format!("Rilo editor -- version {}", RILO_VERSION);
                let cols = config.terminal_cols as usize;

                welcome_text.truncate(cols);

                let mut padding = (cols.saturating_sub(welcome_text.len())) / 2;
                if padding > 0 {
                    queue!(out, Print("~"))?;
                    padding -= 1;
                }

                if padding > 0 {
                    queue!(out, Print(" ".repeat(padding)))?;
                }

                queue!(out, Print(welcome_text))?;
            } else {
                queue!(out, Print("~"))?;
            }
        } else {
            let line = &config.rows[file_row].render;
            let visible: String = line
                .chars()
                .skip(config.col_offset as usize)
                .take(config.terminal_cols as usize)
                .collect();

            queue!(out, Print(visible))?;
        }

        queue!(out, Clear(ClearType::UntilNewLine))?;
        queue!(out, Print("\r\n"))?;
    }
    Ok(())
}

fn draw_status_bar(out: &mut impl Write, config: &EditorConfig) -> io::Result<()> {
    queue!(out, SetAttribute(Attribute::Reverse))?;

    let filename_str: String = if config.filename.is_empty() {
        "[No Name]".to_string()
    } else {
        config.filename.chars().take(64).collect()
    };

    let modified = if config.dirty { "(modified)" } else { "" };

    let display_text = format!(
        "{} - {} lines {}",
        filename_str,
        config.rows.len(),
        modified
    );
    let right_display_text = format!("{}/{}", config.cursor_y + 1, config.rows.len());

    let display_len = display_text.chars().count() as u16;
    let right_len = right_display_text.chars().count() as u16;

    if display_len > config.terminal_cols {
        let truncated: String = display_text
            .chars()
            .take(config.terminal_cols as usize)
            .collect();
        queue!(out, Print(&truncated))?;
    } else {
        queue!(out, Print(&display_text))?;

        let mut current_len = display_len;
        while current_len < config.terminal_cols {
            if config.terminal_cols.saturating_sub(current_len) == right_len {
                queue!(out, Print(&right_display_text))?;
                break;
            } else {
                queue!(out, Print(" "))?;
                current_len += 1;
            }
        }
    }

    queue!(out, SetAttribute(Attribute::Reset))?;
    queue!(out, Print("\r\n"))?;
    Ok(())
}

fn draw_message_bar(out: &mut impl Write, config: &EditorConfig) -> io::Result<()> {
    queue!(out, Clear(ClearType::UntilNewLine))?;

    if let Some(time) = config.status_message_time
        && time.elapsed() < Duration::from_secs(3)
    {
        let message_len = config.status_message.chars().count();
        let cols = config.terminal_cols as usize;
        let print_len = message_len.min(cols);

        let display_message: String = config.status_message.chars().take(print_len).collect();
        if display_message.starts_with("Warning:") {
            queue!(
                out,
                SetAttribute(Attribute::Bold),
                SetForegroundColor(Color::Yellow),
                Print("Warning:"),
                SetAttribute(Attribute::Reset),
                ResetColor,
                Print(&display_message["Warning:".len()..])
            )?;
        } else if display_message.starts_with("Error:") {
            queue!(
                out,
                SetAttribute(Attribute::Bold),
                SetForegroundColor(Color::Red),
                Print("Error:"),
                SetAttribute(Attribute::Reset),
                ResetColor,
                Print(&display_message["Error:".len()..])
            )?;
        } else if display_message.starts_with("Info:") {
            queue!(
                out,
                SetAttribute(Attribute::Bold),
                SetForegroundColor(Color::Blue),
                Print("Info:"),
                SetAttribute(Attribute::Reset),
                ResetColor,
                Print(&display_message["Info:".len()..])
            )?;
        } else {
            queue!(out, Print(display_message))?;
        }
    }

    Ok(())
}

fn refresh_screen(
    config: &mut EditorConfig,
    cursor_override: Option<(u16, u16)>,
) -> io::Result<()> {
    config.scroll();

    let mut out = stdout().lock();

    queue!(out, Hide, MoveTo(0, 0))?;

    draw_rows(&mut out, config)?;
    draw_status_bar(&mut out, config)?;
    draw_message_bar(&mut out, config)?;

    let (target_x, target_y) = match cursor_override {
        Some((col, row)) => (col, row),
        None => (
            config.render_x.saturating_sub(config.col_offset),
            config.cursor_y.saturating_sub(config.row_offset),
        ),
    };

    queue!(out, MoveTo(target_x, target_y,), Show)?;

    out.flush()?;

    Ok(())
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let file_path = args.get(1);

    let mut editor_config = EditorConfig::new()?;

    let mut quit_times = QUIT_TIMES;

    // check for file
    if let Some(path) = file_path {
        editor_config.open(path)?;
    }

    enable_raw_mode()?;

    editor_config.set_status_message("Help: Ctrl-C = Quit | Ctrl-S = Save | Ctrl-F = Find");

    // main loop
    loop {
        refresh_screen(&mut editor_config, None)?;

        if let Event::Key(KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            ..
        }) = event::read()?
        {
            match code {
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    if editor_config.dirty && quit_times > 0 {
                        editor_config.set_status_message(&format!(
                            "Warning: File has unsaved changes. Press Ctrl-C {} more times to quit",
                            quit_times
                        ));
                        quit_times -= 1;
                        continue;
                    }
                    break;
                }

                KeyCode::Char('f') if modifiers.contains(KeyModifiers::CONTROL) => {
                    editor_config.find()?
                }

                KeyCode::Char('s') if modifiers.contains(KeyModifiers::CONTROL) => {
                    editor_config.save()?
                }

                KeyCode::Right | KeyCode::Down => {
                    if !editor_config.search_query.is_empty() {
                        editor_config.find_next(1);
                    } else {
                        editor_config.move_cursor(code);
                    }
                }

                KeyCode::Left | KeyCode::Up => {
                    if !editor_config.search_query.is_empty() {
                        editor_config.find_next(-1);
                    } else {
                        editor_config.move_cursor(code);
                    }
                }

                KeyCode::Home => {
                    editor_config.cursor_x = 0;
                }

                KeyCode::End => {
                    let cursor_y = editor_config.cursor_y as usize;
                    if cursor_y < editor_config.rows.len() {
                        editor_config.cursor_x =
                            editor_config.rows[cursor_y].chars.chars().count() as u16;
                    }
                }

                KeyCode::PageUp => {
                    editor_config.cursor_y = editor_config.row_offset;
                    let term_rows = editor_config.terminal_rows;
                    for _ in 0..term_rows {
                        editor_config.move_cursor(KeyCode::Up);
                    }
                }

                KeyCode::PageDown => {
                    let max_row = editor_config.rows.len() as u16;

                    editor_config.cursor_y =
                        editor_config.row_offset + editor_config.terminal_rows.saturating_sub(3);
                    if editor_config.cursor_y > max_row {
                        editor_config.cursor_y = max_row;
                    }

                    let term_rows = editor_config.terminal_rows;
                    for _ in 0..term_rows {
                        editor_config.move_cursor(KeyCode::Down);
                    }
                }

                KeyCode::Tab => editor_config.insert_char('\t'),

                KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL) => {
                    editor_config.insert_char(c);
                }

                KeyCode::Backspace => {
                    editor_config.del_char();
                }

                KeyCode::Delete => {
                    editor_config.move_cursor(KeyCode::Right);
                    editor_config.del_char();
                }

                KeyCode::Enter => {
                    editor_config.insert_newline();
                }

                KeyCode::Esc => {
                    editor_config.search_query.clear();
                    editor_config.last_match = None;
                    editor_config.search_direction = 1;
                }

                _ => {}
            }
            quit_times = QUIT_TIMES;
        }
    }

    clear_screen()?;

    disable_raw_mode()?;

    Ok(())
}
