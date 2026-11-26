use crossterm::{
    self,
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{
        Attribute::{self, Reset},
        Color, Print, ResetColor, SetAttribute, SetForegroundColor,
    },
    terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode, size},
};
use std::{
    env::{self},
    fs::File,
    io::{BufRead, BufReader, Read},
    ops::RemAssign,
    path::PathBuf,
    time::Instant,
};
use std::{fs, io};
use std::{
    io::{Write, stdout},
    time::Duration,
};

const RILO_VERSION: &str = env!("CARGO_PKG_VERSION");

const HL_HIGHLIGHT_NUMBER: u32 = 1 << 0;
const HL_HIGHLIGHT_STRING: u32 = 1 << 1;
const HL_HIGHLIGHT_PREPOC: u32 = 1 << 2;

const C_HL_EXTENSION: &[&str] = &[".c", ".h", ".cpp"];
const C_HL_KEYWORDS: &[&str] = &[
    "switch",
    "if",
    "while",
    "for",
    "break",
    "continue",
    "return",
    "else",
    "struct",
    "union",
    "typedef",
    "static",
    "enum",
    "class",
    "case",
    "int|",
    "long|",
    "double|",
    "float|",
    "char|",
    "unsigned|",
    "signed|",
    "void|",
];

#[derive(Debug, Clone, Copy, PartialEq)]
enum EditorHighlight {
    Normal = 0,
    Comment,
    MultilineComment,
    Preproc,
    Keyword1,
    Keyword2,
    String,
    Number,
    Match,
}

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
    last_match: Option<(usize, usize)>,
    search_direction: i8,
    syntax: Option<&'static EditorSyntax>,
    tab_stop: usize,
    quit_times: usize,
}

struct EditorRow {
    chars: String,
    render: String,
    highlight: Vec<EditorHighlight>,
    start_in_comment: bool,
    open_comment: bool,
}

struct EditorSyntax {
    filetype: &'static str,
    filematch: &'static [&'static str],
    keywords: &'static [&'static str],
    singleline_comment_start: &'static str,
    multiline_comment_start: &'static str,
    multiline_comment_end: &'static str,
    flags: u32,
}

const HLDB: &[EditorSyntax] = &[EditorSyntax {
    filetype: "c",
    filematch: C_HL_EXTENSION,
    keywords: C_HL_KEYWORDS,
    singleline_comment_start: "//",
    multiline_comment_start: "/*",
    multiline_comment_end: "*/",
    flags: HL_HIGHLIGHT_NUMBER | HL_HIGHLIGHT_STRING | HL_HIGHLIGHT_PREPOC,
}];

impl EditorRow {
    fn new(chars: String, syntax: Option<&EditorSyntax>, tab_stop: usize) -> Self {
        let mut row = Self {
            chars,
            render: String::new(),
            highlight: Vec::new(),
            start_in_comment: false,
            open_comment: false,
        };
        row.update_render(syntax, tab_stop);
        row
    }

    fn cx_to_rx(&self, cursor_x: usize, tab_stop: usize) -> usize {
        let mut render_x: usize = 0;

        for (i, ch) in self.chars.chars().enumerate() {
            if i >= cursor_x {
                break;
            }
            if ch == '\t' {
                render_x += (tab_stop - 1) - (render_x % tab_stop);
            }
            render_x += 1;
        }

        render_x
    }

    fn rx_to_cx(&self, render_x: usize, tab_stop: usize) -> usize {
        let mut cursor_x = 0;

        for (cx, ch) in self.chars.chars().enumerate() {
            if ch == '\t' {
                cursor_x += (tab_stop - 1) - (cursor_x % tab_stop);
            }

            cursor_x += 1;

            if cursor_x > render_x {
                return cx;
            }
        }

        self.chars.chars().count()
    }

    fn update_render(&mut self, syntax: Option<&EditorSyntax>, tab_stop: usize) {
        let tab_count = self.chars.chars().filter(|&c| c == '\t').count();
        self.render.clear();
        self.render.reserve(self.chars.len() + tab_count * 7);

        let mut idx = 0;
        for ch in self.chars.chars() {
            if ch == '\t' {
                self.render.push(' ');
                idx += 1;
                while idx % tab_stop != 0 {
                    self.render.push(' ');
                    idx += 1;
                }
            } else {
                self.render.push(ch);
                idx += 1;
            }
        }

        self.update_syntax(syntax);
    }

    fn insert_char(&mut self, at: usize, c: char, syntax: Option<&EditorSyntax>, tab_stop: usize) {
        let byte_idx = self
            .chars
            .char_indices()
            .nth(at)
            .map_or(self.chars.len(), |(idx, _)| idx);

        self.chars.insert(byte_idx, c);
        self.update_render(syntax, tab_stop);
    }

    fn del_char(&mut self, at: usize, syntax: Option<&EditorSyntax>, tab_stop: usize) {
        if let Some((byte_idx, _)) = self.chars.char_indices().nth(at) {
            self.chars.remove(byte_idx);
            self.update_render(syntax, tab_stop);
        }
    }

    fn update_syntax(&mut self, syntax: Option<&EditorSyntax>) {
        self.highlight = vec![EditorHighlight::Normal; self.render.len()];

        let Some(syntax) = syntax else {
            self.open_comment = false;
            return;
        };

        let highlight_numbers = (syntax.flags & HL_HIGHLIGHT_NUMBER) != 0;
        let highlight_strings = (syntax.flags & HL_HIGHLIGHT_STRING) != 0;
        let highlight_preproc = (syntax.flags & HL_HIGHLIGHT_PREPOC) != 0;

        let keywords: &[&str] = syntax.keywords;

        let scs = syntax.singleline_comment_start.as_bytes();
        let mcs = syntax.multiline_comment_start.as_bytes();
        let mce = syntax.multiline_comment_end.as_bytes();

        let bytes = self.render.as_bytes();

        let mut i = 0;
        let mut prev_sep = true;
        let mut in_string: Option<u8> = None;
        let mut in_comment = self.start_in_comment;

        let first_non_space = bytes.iter().position(|b| !b.is_ascii_whitespace());

        while i < bytes.len() {
            let c = bytes[i];
            let prev_hl = if i > 0 {
                self.highlight[i - 1]
            } else {
                EditorHighlight::Normal
            };

            if !scs.is_empty() && in_string.is_none() && !in_comment && bytes[i..].starts_with(scs)
            {
                self.highlight[i..].fill(EditorHighlight::Comment);
                break;
            }

            if !mcs.is_empty() && !mce.is_empty() && in_string.is_none() {
                if in_comment {
                    self.highlight[i] = EditorHighlight::MultilineComment;
                    if bytes[i..].starts_with(mce) {
                        self.highlight[i..i + mce.len()].fill(EditorHighlight::MultilineComment);
                        i += mce.len();
                        in_comment = false;
                        prev_sep = true;
                    } else {
                        i += 1;
                    }
                    continue;
                } else if bytes[i..].starts_with(mcs) {
                    self.highlight[i..i + mcs.len()].fill(EditorHighlight::MultilineComment);
                    i += mcs.len();
                    in_comment = true;
                    continue;
                }
            }

            if highlight_preproc && c == b'#' && Some(i) == first_non_space {
                let mut end = i + 1;

                while end < bytes.len() && bytes[end].is_ascii_whitespace() {
                    end += 1;
                }
                while end < bytes.len() && bytes[end].is_ascii_alphabetic() {
                    end += 1;
                }

                self.highlight[i..end].fill(EditorHighlight::Preproc);

                if bytes[i + 1..end].trim_ascii() == b"include" {
                    let mut j = end;
                    while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    if j < bytes.len()
                        && bytes[j] == b'<'
                        && let Some(rel) = bytes[j..].iter().position(|&b| b == b'>')
                    {
                        self.highlight[j..=j + rel].fill(EditorHighlight::String);
                        end = j + rel + 1;
                    }
                }

                i = end;
                prev_sep = true;
                continue;
            }

            if highlight_strings {
                if let Some(quote) = in_string {
                    self.highlight[i] = EditorHighlight::String;

                    if c == b'\\' && i + 1 < bytes.len() {
                        self.highlight[i + 1] = EditorHighlight::String;
                        i += 2;
                        continue;
                    }

                    if c == quote {
                        in_string = None;
                    }

                    i += 1;
                    prev_sep = true;
                    continue;
                } else if c == b'"' || c == b'\'' {
                    in_string = Some(c);
                    self.highlight[i] = EditorHighlight::String;
                    i += 1;
                    prev_sep = false;
                    continue;
                }
            }

            if highlight_numbers {
                let is_num_char = (c.is_ascii_digit()
                    && (prev_sep || prev_hl == EditorHighlight::Number))
                    || (c == b'.' && prev_hl == EditorHighlight::Number)
                    || (prev_hl == EditorHighlight::Number
                        && matches!(c, b'x' | b'X' | b'a'..=b'f' | b'A'..=b'F' | b'u' | b'U' | b'l' | b'L'));

                if is_num_char {
                    self.highlight[i] = EditorHighlight::Number;
                    i += 1;
                    prev_sep = false;
                    continue;
                }
            }

            if prev_sep {
                let mut matched = false;

                for kw in keywords {
                    let (word, kw2) = match kw.strip_suffix('|') {
                        Some(w) => (w, true),
                        None => (*kw, false),
                    };
                    let word = word.as_bytes();
                    let klen = word.len();

                    if bytes[i..].starts_with(word)
                        && bytes.get(i + klen).is_none_or(|&b| is_separator(b as char))
                    {
                        let kind = if kw2 {
                            EditorHighlight::Keyword2
                        } else {
                            EditorHighlight::Keyword1
                        };
                        self.highlight[i..i + klen].fill(kind);
                        i += klen;
                        matched = true;
                        break;
                    }
                }

                if matched {
                    prev_sep = false;
                    continue;
                }
            }

            prev_sep = is_separator(c as char);
            i += 1;
        }
        self.open_comment = in_comment;
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
            syntax: None,
            tab_stop: 4,
            quit_times: 1,
        })
    }

    fn open(&mut self, path: &str) -> io::Result<()> {
        self.filename = path.to_string();

        self.select_syntax_highlight();
        let content = fs::read_to_string(path)?;

        self.rows = content
            .lines()
            .map(|line| EditorRow::new(line.to_string(), self.syntax, self.tab_stop))
            .collect();

        self.update_all_syntax();
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
            self.render_x = self.rows[self.cursor_y as usize]
                .cx_to_rx(self.cursor_x as usize, self.tab_stop) as u16;
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
            self.rows
                .push(EditorRow::new(String::new(), self.syntax, self.tab_stop));
        }

        self.rows[self.cursor_y as usize].insert_char(
            self.cursor_x as usize,
            c,
            self.syntax,
            self.tab_stop,
        );
        self.dirty = true;
        self.update_all_syntax();
        self.cursor_x += 1;
    }

    fn del_char(&mut self) {
        let cx = self.cursor_x as usize;
        let cy = self.cursor_y as usize;

        if cy == self.rows.len() || (cx == 0 && cy == 0) {
            return;
        }

        if cx > 0 {
            self.rows[cy].del_char(cx - 1, self.syntax, self.tab_stop);
            self.cursor_x -= 1;
        } else {
            let current_row = self.rows.remove(cy);
            let prev_row = &mut self.rows[cy - 1];

            self.cursor_x = prev_row.chars.chars().count() as u16;

            prev_row.chars.push_str(&current_row.chars);
            prev_row.update_render(self.syntax, self.tab_stop);

            self.cursor_y -= 1;
        }

        self.update_all_syntax();
        self.dirty = true;
    }

    fn insert_newline(&mut self) {
        let cx = self.cursor_x as usize;
        let cy = self.cursor_y as usize;

        if cx == 0 {
            // if cursor is at the start of a line (or the bottom ~)
            self.rows.insert(
                cy,
                EditorRow::new(String::new(), self.syntax, self.tab_stop),
            );
        } else {
            // if cursor at the middle of a line
            let byte_idx = self.rows[cy]
                .chars
                .char_indices()
                .nth(cx)
                .map_or(self.rows[cy].chars.len(), |(idx, _)| idx);

            let right_half = self.rows[cy].chars.split_off(byte_idx);
            self.rows[cy].update_render(self.syntax, self.tab_stop);

            self.rows.insert(
                cy + 1,
                EditorRow::new(right_half, self.syntax, self.tab_stop),
            );
        }

        self.update_all_syntax();
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
            return self.save_as();
        }

        self.write_file()
    }

    fn save_as(&mut self) -> io::Result<()> {
        let Some(name) = self.prompt("Save as: {}")? else {
            self.set_status_message("Warning: Save aborted");
            return Ok(());
        };

        self.filename = name;

        self.select_syntax_highlight();
        self.update_all_syntax();

        self.write_file()
    }

    fn write_file(&mut self) -> io::Result<()> {
        let content = self.rows_to_string();
        let len = content.len();

        match fs::write(&self.filename, content) {
            Ok(()) => {
                self.set_status_message(&format!("Info: {} bytes written to disk", len));
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

        let len = self.rows.len();
        let (mut row_idx, mut search_from_byte) = match self.last_match {
            Some((r, byte_idx)) => (r, byte_idx),
            None => (self.cursor_y.min((len - 1) as u16) as usize, 0),
        };

        for _ in 0..len {
            let render = &self.rows[row_idx].render;

            let found_byte = if direction == 1 {
                let start_byte = if self.last_match.is_some() {
                    render[search_from_byte..]
                        .chars()
                        .next()
                        .map_or(render.len(), |c| search_from_byte + c.len_utf8())
                } else {
                    search_from_byte
                };
                render
                    .get(start_byte..)
                    .and_then(|tail| tail.find(&self.search_query))
                    .map(|rel| start_byte + rel)
            } else {
                let end_byte = search_from_byte.min(render.len());
                render
                    .get(..end_byte)
                    .and_then(|head| head.rfind(&self.search_query))
            };

            if let Some(match_byte) = found_byte {
                let match_rx = render[..match_byte].chars().count();
                self.cursor_y = row_idx as u16;
                self.cursor_x = self.rows[row_idx].rx_to_cx(match_rx, self.tab_stop) as u16;
                self.last_match = Some((row_idx, match_byte));
                self.search_direction = direction;
                self.highlight_match();
                return;
            }

            if direction == 1 {
                row_idx = (row_idx + 1) % len;
                search_from_byte = 0;
            } else {
                row_idx = (row_idx + len - 1) % len;
                search_from_byte = self.rows[row_idx].render.len();
            }
        }

        self.set_status_message("Warning: No match found");
    }

    fn highlight_match(&mut self) {
        self.update_all_syntax();

        if self.search_query.is_empty() {
            return;
        }

        let query_len = self.search_query.len();

        for row in &mut self.rows {
            let mut from = 0;
            while let Some(rel_idx) = row.render[from..].find(&self.search_query) {
                let start = from + rel_idx;
                let end = (start + query_len).min(row.highlight.len());
                for i in start..end {
                    row.highlight[i] = EditorHighlight::Match;
                }
                from = end;
            }
        }
    }

    fn select_syntax_highlight(&mut self) {
        self.syntax = None;
        if self.filename.is_empty() {
            return;
        }

        let ext = self.filename.rsplit_once('.').map(|(_, after)| after);

        for syntax in HLDB {
            for pattern in syntax.filematch {
                let matched = if let Some(pat_ext) = pattern.strip_prefix('.') {
                    ext == Some(pat_ext)
                } else {
                    self.filename.contains(pattern)
                };

                if matched {
                    self.syntax = Some(syntax);

                    self.update_all_syntax();

                    return;
                }
            }
        }
    }

    fn update_all_syntax(&mut self) {
        let syntax = self.syntax;
        let mut open = false;
        for row in &mut self.rows {
            row.start_in_comment = open;
            row.update_syntax(syntax);
            open = row.open_comment;
        }
    }

    fn config_path() -> io::Result<PathBuf> {
        let home = env::var_os("HOME")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME not set"))?;
        Ok(PathBuf::from(home).join(".rilorc"))
    }

    fn parse_config(&mut self) -> io::Result<()> {
        let path = EditorConfig::config_path()?;

        let file = File::open(path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            eprintln!("{line}");
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = trimmed.split_once('=') {
                let value = value.trim();

                match key.trim() {
                    "QUIT_TIMES" => {
                        self.quit_times = value
                            .parse()
                            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                    }
                    "TAB_STOP" => {
                        self.tab_stop = value
                            .parse()
                            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

fn clear_screen() -> io::Result<()> {
    execute!(io::stdout(), Clear(ClearType::All), MoveTo(0, 0)) // clear screen and put cursor on the top left corner
}

fn syntax_to_color(highlight: EditorHighlight) -> Color {
    match highlight {
        EditorHighlight::Normal => Color::Reset, // base color
        EditorHighlight::Comment | EditorHighlight::MultilineComment => Color::Cyan,
        EditorHighlight::Preproc => Color::AnsiValue(81), // match vim color
        EditorHighlight::Keyword1 => Color::Yellow,       // control flow
        EditorHighlight::Keyword2 => Color::Green,        // types
        EditorHighlight::String => Color::Magenta,
        EditorHighlight::Number => Color::DarkMagenta,
        EditorHighlight::Match => Color::Yellow,
    }
}

fn welcome_lines() -> Vec<Vec<(String, Color)>> {
    let text = Color::Reset;
    let key = Color::Yellow;
    let s = |t: &str, c: Color| (t.to_string(), c);

    let mut lines = vec![
        vec![s("RILO - a terminal text editor", Color::Cyan)],
        vec![s(&format!("version {}", RILO_VERSION), text)],
        vec![s("open source, written in Rust", text)],
        vec![s("https://github.com/wirenux/rilo", Color::Blue)],
        vec![s("(a ~/.rilorc might be useful :D)", Color::Grey)],
        vec![],
    ];

    let keys = [
        ("Ctrl-S", "to save"),
        ("Ctrl-Shift-S", "to save as..."),
        ("  (Ctrl-O)", "to save as..."),
        ("Ctrl-F", "to search"),
        ("Ctrl-C", "to quit"),
        ("Esc", "to clear search/prompt"),
    ];

    // center everything
    let key_w = keys
        .iter()
        .map(|(k, _)| k.chars().count())
        .max()
        .unwrap_or(0);
    let desc_w = keys
        .iter()
        .map(|(_, d)| d.chars().count())
        .max()
        .unwrap_or(0);

    for (k, d) in keys {
        lines.push(vec![
            s("type ", text),
            s(&format!("{:<key_w$}", k), key),
            s(&format!("  {:<desc_w$}", d), text),
        ]);
    }

    lines
}

fn draw_welcome_lines(
    out: &mut impl Write,
    line: &[(String, Color)],
    cols: usize,
) -> io::Result<()> {
    if line.is_empty() {
        return queue!(
            out,
            SetForegroundColor(Color::DarkBlue),
            Print("~"),
            SetForegroundColor(Color::Reset)
        );
    }

    let width: usize = line.iter().map(|(t, _)| t.chars().count()).sum();
    let lead = cols.saturating_sub(width) / 2;
    let mut padding = lead;

    if padding > 0 {
        queue!(
            out,
            SetForegroundColor(Color::DarkBlue),
            Print("~"),
            SetForegroundColor(Color::Reset)
        )?;
        padding -= 1;
    }

    if padding > 0 {
        queue!(out, Print(" ".repeat(padding)))?;
    }

    let mut remaining = cols.saturating_sub(lead);
    for (text, color) in line {
        let part: String = text.chars().take(remaining).collect();
        remaining -= part.chars().count();
        queue!(out, SetForegroundColor(*color), Print(part))?;
    }

    queue!(out, ResetColor)
}

fn draw_rows(out: &mut impl Write, config: &EditorConfig) -> io::Result<()> {
    let welcome = welcome_lines();
    let n = welcome.len();
    let cols = config.terminal_cols as usize;

    let show_welcome = config.rows.is_empty() && (config.terminal_rows as usize) > n + 2;
    let welcome_start = (config.terminal_rows as usize).saturating_sub(n) / 3;

    for y in 0..config.terminal_rows {
        let file_row = (y as usize) + (config.row_offset as usize);
        if file_row >= config.rows.len() {
            let welcome_idx = (y as usize).checked_sub(welcome_start).filter(|&k| k < n);

            if show_welcome && let Some(k) = welcome_idx {
                draw_welcome_lines(out, &welcome[k], cols)?;
            } else {
                queue!(
                    out,
                    SetForegroundColor(Color::DarkBlue),
                    Print("~"),
                    SetForegroundColor(Color::Reset)
                )?;
            }
        } else {
            let line = &config.rows[file_row].render;
            let start = config.col_offset as usize;
            let end = start + config.terminal_cols as usize;

            for (byte_idx, ch) in line.char_indices() {
                if byte_idx < start {
                    continue;
                }

                if byte_idx >= end {
                    break;
                }

                let highlight = config.rows[file_row].highlight[byte_idx];

                if ch.is_control() {
                    let symbol = if (ch as u32) <= 26 {
                        (b'@' + ch as u8) as char
                    } else {
                        '?'
                    };

                    queue!(
                        out,
                        SetAttribute(Attribute::Reverse),
                        Print(symbol),
                        SetAttribute(Attribute::NoReverse)
                    )?;
                } else {
                    if highlight == EditorHighlight::Match {
                        queue!(out, SetAttribute(Attribute::Reverse))?;
                    }
                    queue!(
                        out,
                        SetForegroundColor(syntax_to_color(highlight)),
                        Print(ch)
                    )?;
                    if highlight == EditorHighlight::Match {
                        queue!(out, SetAttribute(Attribute::NoReverse))?;
                    }
                }
            }

            queue!(out, ResetColor)?;
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

    let filetype = config.syntax.map(|s| s.filetype).unwrap_or("no filetype");

    let display_text = format!(
        "{} - {} lines {}",
        filename_str,
        config.rows.len(),
        modified
    );

    let right_display_text = format!(
        "{} | {}/{}",
        filetype,
        config.cursor_y + 1,
        config.rows.len()
    );

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
        } else if display_message.starts_with("Help:") {
            queue!(
                out,
                SetAttribute(Attribute::Bold),
                SetForegroundColor(Color::Cyan),
                Print("Help:"),
                SetAttribute(Attribute::Reset),
                ResetColor,
                Print(&display_message["Help:".len()..])
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

fn is_separator(c: char) -> bool {
    c.is_whitespace() || ",.()+-/*=~%<>[];{}:&|!?#".contains(c)
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let file_path = args.get(1);

    let mut editor_config = EditorConfig::new()?;

    enable_raw_mode()?;

    let _ = editor_config.parse_config();

    // check for file
    if let Some(path) = file_path {
        editor_config.open(path)?;
    }

    if editor_config.status_message.is_empty() {
        editor_config.set_status_message("Help: Ctrl-C = Quit | Ctrl-S = Save | Ctrl-F = Find");
    }

    let mut quit_times = editor_config.quit_times;

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

                KeyCode::Char('s' | 'S')
                    if modifiers.contains(KeyModifiers::CONTROL)
                        && modifiers.contains(KeyModifiers::SHIFT) =>
                {
                    editor_config.save_as()?
                }

                KeyCode::Char('o') if modifiers.contains(KeyModifiers::CONTROL) => {
                    editor_config.save_as()?
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

                KeyCode::Char('a') if modifiers.contains(KeyModifiers::CONTROL) => {
                    editor_config.cursor_x = 0;
                }

                KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                    let cursor_y = editor_config.cursor_y as usize;
                    if cursor_y < editor_config.rows.len() {
                        editor_config.cursor_x =
                            editor_config.rows[cursor_y].chars.chars().count() as u16;
                    }
                }

                KeyCode::Char('w') if modifiers.contains(KeyModifiers::CONTROL) => {
                    editor_config.cursor_y = editor_config.row_offset;
                    let term_rows = editor_config.terminal_rows;
                    for _ in 0..term_rows {
                        editor_config.move_cursor(KeyCode::Up);
                    }
                }

                KeyCode::Char('x') if modifiers.contains(KeyModifiers::CONTROL) => {
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
                    editor_config.update_all_syntax();
                }

                _ => {}
            }
            quit_times = editor_config.quit_times;
        }
    }

    clear_screen()?;

    disable_raw_mode()?;

    Ok(())
}
