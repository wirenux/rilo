use crossterm::{
    self,
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{Attribute, Color, Print, SetAttribute, Stylize},
    terminal::{disable_raw_mode, enable_raw_mode, size, Clear, ClearType},
};
use std::env::{self, current_exe};
use std::io::{stdout, Write};
use std::{fs, io};

const RILO_VERSION: &str = env!("CARGO_PKG_VERSION");
const TAB_STOP: usize = 8;

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
}

impl EditorConfig {
    fn new() -> io::Result<Self> {
        let (cols, mut rows) = size()?;
        rows = rows.saturating_sub(1);

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
        })
    }

    fn open(&mut self, path: &str) -> io::Result<()> {
        self.filename = path.to_string();

        let content = fs::read_to_string(path)?;
        self.rows = content
            .lines()
            .map(|line| EditorRow::new(line.to_string()))
            .collect();
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

    let display_text = format!("{} - {} lines", filename_str, config.rows.len());
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
    Ok(())
}

fn refresh_screen(config: &mut EditorConfig) -> io::Result<()> {
    config.scroll();

    let mut out = stdout().lock();

    queue!(out, Hide, MoveTo(0, 0))?;
    draw_rows(&mut out, config)?;
    draw_status_bar(&mut out, config)?;
    queue!(
        out,
        MoveTo(
            config.render_x - config.col_offset,
            config.cursor_y - config.row_offset
        ),
        Show
    )?;

    out.flush()?;

    Ok(())
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let file_path = args.get(1);

    let mut editor_config = EditorConfig::new()?;

    // check for file
    if let Some(path) = file_path {
        editor_config.open(path)?;
    }

    enable_raw_mode()?;

    // main loop
    loop {
        refresh_screen(&mut editor_config)?;

        if let Event::Key(KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            ..
        }) = event::read()?
        {
            match code {
                KeyCode::Char('q') => break,
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Char('h' | 'j' | 'k' | 'l')
                | KeyCode::Left
                | KeyCode::Down
                | KeyCode::Up
                | KeyCode::Right => editor_config.move_cursor(code),
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

                _ => {}
            }
        }
    }

    clear_screen()?;

    disable_raw_mode()?;

    Ok(())
}
