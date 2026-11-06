//! Legacy xterm key encoding, invoked only by the supervisor worker.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn encode(key: KeyEvent, application_cursor: bool) -> Result<Vec<u8>, &'static str> {
    let modifiers = key.modifiers;
    if modifiers.intersects(KeyModifiers::SUPER | KeyModifiers::HYPER | KeyModifiers::META) {
        return Err("This terminal cannot forward Super/Hyper/Meta shortcuts");
    }
    let parameter = 1
        + u8::from(modifiers.contains(KeyModifiers::SHIFT))
        + 2 * u8::from(modifiers.contains(KeyModifiers::ALT))
        + 4 * u8::from(modifiers.contains(KeyModifiers::CONTROL));
    let mut bytes = match key.code {
        KeyCode::Char(c) => {
            let mut bytes = if modifiers.contains(KeyModifiers::CONTROL) {
                let value = match c {
                    '@' | ' ' | '2' => 0,
                    'a'..='z' => c as u8 - b'a' + 1,
                    'A'..='Z' => c as u8 - b'A' + 1,
                    '[' | '3' => 27,
                    '\\' | '4' => 28,
                    ']' | '5' => 29,
                    '^' | '6' => 30,
                    '_' | '7' | '/' => 31,
                    '?' | '8' => 127,
                    _ => return Err("Control shortcut has no legacy terminal encoding"),
                };
                vec![value]
            } else {
                c.to_string().into_bytes()
            };
            if modifiers.contains(KeyModifiers::ALT) {
                bytes.insert(0, 27);
            }
            return Ok(bytes);
        }
        KeyCode::Null => vec![0],
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![127],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => return Ok(b"\x1b[Z".to_vec()),
        KeyCode::Esc => vec![27],
        KeyCode::Up
        | KeyCode::Down
        | KeyCode::Right
        | KeyCode::Left
        | KeyCode::Home
        | KeyCode::End => {
            let suffix = match key.code {
                KeyCode::Up => 'A',
                KeyCode::Down => 'B',
                KeyCode::Right => 'C',
                KeyCode::Left => 'D',
                KeyCode::Home => 'H',
                _ => 'F',
            };
            return Ok(if parameter == 1 {
                format!("\x1b{}{suffix}", if application_cursor { 'O' } else { '[' })
            } else {
                format!("\x1b[1;{parameter}{suffix}")
            }
            .into_bytes());
        }
        KeyCode::Insert
        | KeyCode::Delete
        | KeyCode::PageUp
        | KeyCode::PageDown
        | KeyCode::F(1..=12) => {
            let number = match key.code {
                KeyCode::Insert => 2,
                KeyCode::Delete => 3,
                KeyCode::PageUp => 5,
                KeyCode::PageDown => 6,
                KeyCode::F(n) => {
                    [11, 12, 13, 14, 15, 17, 18, 19, 20, 21, 23, 24][usize::from(n - 1)]
                }
                _ => unreachable!(),
            };
            if let KeyCode::F(n @ 1..=4) = key.code {
                let suffix = char::from(b'P' + n - 1);
                return Ok(if parameter == 1 {
                    format!("\x1bO{suffix}")
                } else {
                    format!("\x1b[1;{parameter}{suffix}")
                }
                .into_bytes());
            }
            return Ok(if parameter == 1 {
                format!("\x1b[{number}~")
            } else {
                format!("\x1b[{number};{parameter}~")
            }
            .into_bytes());
        }
        _ => return Err("Key has no supported legacy terminal encoding"),
    };
    if modifiers.contains(KeyModifiers::ALT) {
        bytes.insert(0, 27);
    }
    Ok(bytes)
}
