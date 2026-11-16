use crossterm::{
    self,
    cursor::{position, Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    style::Print,
    terminal::{disable_raw_mode, enable_raw_mode, size, Clear, ClearType},
};
use std::env;
use std::io::{stdout, Write};
use std::{fs, io};

const RILO_VERSION: &str = env!("CARGO_PKG_VERSION");

struct EditorConfig {
    terminal_cols: u16,
    terminal_rows: u16,
    cursor_x: u16,
    cursor_y: u16,
    rows: Vec<EditorRow>,
}

struct EditorRow {
    chars: String,
}

impl EditorConfig {
    fn new() -> io::Result<Self> {
        let (cols, rows) = size()?;
        Ok(Self {
            terminal_cols: cols,
            terminal_rows: rows,
            cursor_y: 0,
            cursor_x: 0,
            rows: Vec::new(),
        })
    }

    fn open(&mut self, path: &str) -> io::Result<()> {
        let content = fs::read_to_string(path)?;
        self.rows = content
            .lines()
            .map(|line| EditorRow {
                chars: line.to_string(),
            })
            .collect();
        Ok(())
    }

    fn move_cursor(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('h') | KeyCode::Left => {
                self.cursor_x = self.cursor_x.saturating_sub(1);
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.cursor_y < self.terminal_rows.saturating_sub(1) {
                    self.cursor_y += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.cursor_y = self.cursor_y.saturating_sub(1);
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if self.cursor_x < self.terminal_cols.saturating_sub(1) {
                    self.cursor_x += 1;
                }
            }

            _ => {}
        }
    }
}

fn clear_screen() -> io::Result<()> {
    execute!(io::stdout(), Clear(ClearType::All), MoveTo(0, 0)) // clear screen and put cursor on the top left corner
}

fn draw_rows(out: &mut impl Write, config: &EditorConfig) -> io::Result<()> {
    for y in 0..config.terminal_rows {
        if y >= config.rows.len() as u16 {
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
            let line = &config.rows[y as usize].chars;
            let visible: String = line.chars().take(config.terminal_cols as usize).collect();
            queue!(out, Print(visible))?;
        }

        queue!(out, Clear(ClearType::UntilNewLine))?;

        if y < config.terminal_rows - 1 {
            queue!(out, Print("\r\n"))?;
        }
    }
    Ok(())
}

fn refresh_screen(config: &EditorConfig) -> io::Result<()> {
    let mut out = stdout().lock();

    queue!(out, Hide, MoveTo(0, 0))?;
    draw_rows(&mut out, config)?;
    queue!(out, MoveTo(config.cursor_x, config.cursor_y), Show)?;

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
        refresh_screen(&editor_config)?;

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
                    editor_config.cursor_x = editor_config.terminal_cols.saturating_sub(1);
                }
                KeyCode::PageUp => {
                    for _ in 0..editor_config.terminal_rows {
                        editor_config.move_cursor(KeyCode::Up);
                    }
                }
                KeyCode::PageDown => {
                    for _ in 0..editor_config.terminal_rows {
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
