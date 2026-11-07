//! A bounded, inert projection of terminal output, independent of native records.
//! Locations refer to the authoritative raw byte stream, never rendered row numbers.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReadablePage {
    pub version: u32,
    pub raw_start: u64,
    pub raw_end: u64,
    pub committed: u64,
    pub text: String,
    pub passages: Vec<Passage>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Passage {
    pub raw_start: u64,
    pub raw_end: u64,
    pub text_start: usize,
    pub text_end: usize,
    pub provisional: bool,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Projection {
    mode: Mode,
    utf8: Vec<u8>,
    utf8_start: u64,
    line: Vec<(char, u64, u64)>,
    cursor: usize,
    carriage: bool,
    row: Option<u16>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
enum Mode {
    #[default]
    Text,
    Escape,
    Csi(Vec<u8>),
    String,
    StringEscape,
}
impl Projection {
    /// Feed consecutive raw bytes. Only lines completed in this page's raw range
    /// are emitted; an unfinished line remains in the checkpoint, not a guess.
    pub fn feed(
        &mut self,
        raw: &[u8],
        start: u64,
        range: std::ops::Range<u64>,
        page: &mut ReadablePage,
    ) {
        page.version = 1;
        for (index, &byte) in raw.iter().enumerate() {
            let position = start + index as u64;
            self.byte(byte, position, &range, page);
        }
    }
    /// A page preview does not alter the checkpoint. Subsequent output may redraw
    /// this last line; completed passages retain their original raw locations.
    pub fn preview(&self, end: u64, range: std::ops::Range<u64>, page: &mut ReadablePage) {
        let mut copy = self.clone();
        let count = page.passages.len();
        copy.flush(end, &range, page);
        for passage in &mut page.passages[count..] {
            passage.provisional = true;
        }
    }
    fn flush(&mut self, end: u64, range: &std::ops::Range<u64>, page: &mut ReadablePage) {
        if !self.line.is_empty() && end > range.start && end <= range.end {
            let text_start = page.text.len();
            let raw_start = self.line.iter().map(|cell| cell.1).min().unwrap();
            let raw_end = self.line.iter().map(|cell| cell.2).max().unwrap().max(end);
            page.text.extend(self.line.iter().map(|cell| cell.0));
            page.text.push('\n');
            page.passages.push(Passage {
                raw_start,
                raw_end,
                text_start,
                text_end: page.text.len(),
                provisional: false,
            });
        }
        self.line.clear();
        self.cursor = 0;
        self.carriage = false;
    }
    fn character(
        &mut self,
        character: char,
        start: u64,
        end: u64,
        range: &std::ops::Range<u64>,
        page: &mut ReadablePage,
    ) {
        if self.carriage {
            self.cursor = 0;
            self.carriage = false;
        }
        if self.cursor == 2048 {
            self.flush(start, range, page);
        }
        let character = if character.is_control()
            || matches!(character, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        {
            '�'
        } else {
            character
        };
        let cell = (character, start, end);
        if self.cursor < self.line.len() {
            self.line[self.cursor] = cell;
        } else {
            self.line.push(cell);
        }
        self.cursor += 1;
    }
    fn horizontal(&mut self, column: usize, position: u64) {
        self.carriage = false;
        self.cursor = column.min(2047);
        while self.line.len() < self.cursor {
            self.line.push((' ', position, position + 1));
        }
    }
    fn byte(
        &mut self,
        byte: u8,
        position: u64,
        range: &std::ops::Range<u64>,
        page: &mut ReadablePage,
    ) {
        let mode = std::mem::take(&mut self.mode);
        match mode {
            Mode::String => {
                self.mode = if byte == 0x1b {
                    Mode::StringEscape
                } else if byte == 7 {
                    Mode::Text
                } else {
                    Mode::String
                }
            }
            Mode::StringEscape => {
                self.mode = if byte == b'\\' || byte == 7 {
                    Mode::Text
                } else if byte == 0x1b {
                    Mode::StringEscape
                } else {
                    Mode::String
                }
            }
            Mode::Escape => {
                self.mode = match byte {
                    b'[' => Mode::Csi(Vec::new()),
                    b']' | b'P' | b'_' | b'^' | b'X' => Mode::String,
                    0x20..=0x2f => Mode::Escape,
                    _ => Mode::Text,
                }
            }
            Mode::Csi(mut parameters) => {
                if byte == 0x1b {
                    self.mode = Mode::Escape;
                } else if (0x40..=0x7e).contains(&byte) {
                    let numbers = std::str::from_utf8(&parameters)
                        .unwrap_or("")
                        .split(';')
                        .map(|number| number.parse::<u16>().unwrap_or(1).max(1))
                        .collect::<Vec<_>>();
                    let first = numbers.first().copied().unwrap_or(1) as usize;
                    match byte {
                        b'G' | b'`' => self.horizontal(first - 1, position),
                        b'C' | b'a' => self.horizontal(self.cursor.saturating_add(first), position),
                        b'D' => self.horizontal(self.cursor.saturating_sub(first), position),
                        b'H' | b'f' => {
                            let row = first as u16;
                            if self.row != Some(row) {
                                self.flush(position + 1, range, page);
                            }
                            self.row = Some(row);
                            self.horizontal(
                                numbers.get(1).copied().unwrap_or(1) as usize - 1,
                                position,
                            );
                        }
                        b'm' | b'h' | b'l' | b'n' | b'c' | b'q' | b't' => {}
                        b'K' if parameters == b"2" => {
                            self.line.clear();
                            self.cursor = 0;
                        }
                        b'K' => {
                            self.line.truncate(self.cursor);
                        }
                        // Cursor-addressed redraws are preserved as separate lines,
                        // rather than pretending to reconstruct a structured chat.
                        _ => self.flush(position + 1, range, page),
                    }
                } else {
                    if parameters.len() < 32 {
                        parameters.push(byte);
                    }
                    self.mode = Mode::Csi(parameters);
                }
            }
            Mode::Text => {
                if !self.utf8.is_empty() {
                    if byte & 0xc0 == 0x80 {
                        self.utf8.push(byte);
                        if let Ok(text) = std::str::from_utf8(&self.utf8) {
                            let character = text.chars().next().unwrap();
                            self.character(character, self.utf8_start, position + 1, range, page);
                            self.utf8.clear();
                        } else if std::str::from_utf8(&self.utf8)
                            .unwrap_err()
                            .error_len()
                            .is_some()
                        {
                            self.character('�', self.utf8_start, position + 1, range, page);
                            self.utf8.clear();
                        }
                        return;
                    }
                    self.character('�', self.utf8_start, position, range, page);
                    self.utf8.clear();
                }
                match byte {
                    0x1b => self.mode = Mode::Escape,
                    b'\n' => self.flush(position + 1, range, page),
                    b'\r' => self.carriage = true,
                    8 => self.cursor = self.cursor.saturating_sub(1),
                    b'\t' => self.character(' ', position, position + 1, range, page),
                    0..=31 | 127 => {}
                    32..=126 => self.character(byte as char, position, position + 1, range, page),
                    0xc2..=0xf4 => {
                        self.utf8_start = position;
                        self.utf8.push(byte);
                    }
                    _ => self.character('�', position, position + 1, range, page),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_horizontal_and_same_row_cursor_addressing_keep_sentences_readable() {
        let raw = b"\x1b[2GQuick\x1b[8Gsafety\x1b[15Gcheck:\r\n\x1b[4;1HDo\x1b[4;4Hyou\x1b[4;8Hagree?\x1b[5;1HAnswer\n";
        let mut projection = Projection::default();
        let mut page = ReadablePage::default();
        projection.feed(raw, 0, 0..raw.len() as u64, &mut page);
        assert_eq!(page.text, " Quick safety check:\nDo you agree?\nAnswer\n");
    }
    #[test]
    fn unicode_controls_are_inert_even_when_utf8_is_split_between_pages() {
        let mut projection = Projection::default();
        let mut first = ReadablePage::default();
        projection.feed(&[b'A', 0xc2], 0, 0..2, &mut first);
        let mut second = ReadablePage::default();
        projection.feed(&[0x9b, b'B', b'\n'], 2, 2..5, &mut second);
        assert_eq!(second.text, "A�B\n");
        assert_eq!(second.passages[0].raw_start, 0);
        assert_eq!(second.passages[0].raw_end, 5);
    }
    #[test]
    fn readable_history_handles_fragmented_utf8_controls_and_redraw_without_executing_them() {
        let raw = "Prompt: café\r\n\x1b[32mAnswer\x1b[0m\nold\rnew\x1b[K\n\x1b]52;c;hidden-secret\x07command output\n".as_bytes();
        let mut projection = Projection::default();
        let mut page = ReadablePage::default();
        for (position, byte) in raw.iter().enumerate() {
            projection.feed(&[*byte], position as u64, 0..raw.len() as u64, &mut page);
        }
        assert_eq!(page.text, "Prompt: café\nAnswer\nnew\ncommand output\n");
        assert_eq!(page.passages[0].raw_start, 0);
        assert_eq!(page.passages[0].raw_end, 15);
    }
}
