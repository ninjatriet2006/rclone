use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};

pub fn handle_input_key(
    key: &KeyEvent,
    input_buffer: &mut String,
    edit_cursor_idx: &mut usize,
) -> bool {
    let chars: Vec<char> = input_buffer.chars().collect();
    let char_count = chars.len();
    
    if *edit_cursor_idx > char_count {
        *edit_cursor_idx = char_count;
    }

    match key.code {
        KeyCode::Left => {
            if *edit_cursor_idx > 0 {
                *edit_cursor_idx -= 1;
            }
            true
        }
        KeyCode::Right => {
            if *edit_cursor_idx < char_count {
                *edit_cursor_idx += 1;
            }
            true
        }
        KeyCode::Home => {
            *edit_cursor_idx = 0;
            true
        }
        KeyCode::End => {
            *edit_cursor_idx = char_count;
            true
        }
        KeyCode::Backspace => {
            if *edit_cursor_idx > 0 {
                let mut new_chars = Vec::with_capacity(char_count.saturating_sub(1));
                new_chars.extend_from_slice(&chars[0..*edit_cursor_idx - 1]);
                new_chars.extend_from_slice(&chars[*edit_cursor_idx..]);
                *input_buffer = new_chars.into_iter().collect();
                *edit_cursor_idx -= 1;
            }
            true
        }
        KeyCode::Delete => {
            if *edit_cursor_idx < char_count {
                let mut new_chars = Vec::with_capacity(char_count.saturating_sub(1));
                new_chars.extend_from_slice(&chars[0..*edit_cursor_idx]);
                new_chars.extend_from_slice(&chars[*edit_cursor_idx + 1..]);
                *input_buffer = new_chars.into_iter().collect();
            }
            true
        }
        KeyCode::Char(c) => {
            let has_modifiers = key.modifiers.contains(KeyModifiers::CONTROL) 
                || key.modifiers.contains(KeyModifiers::ALT);
            if !has_modifiers {
                let mut new_chars = Vec::with_capacity(char_count + 1);
                new_chars.extend_from_slice(&chars[0..*edit_cursor_idx]);
                new_chars.push(c);
                new_chars.extend_from_slice(&chars[*edit_cursor_idx..]);
                *input_buffer = new_chars.into_iter().collect();
                *edit_cursor_idx += 1;
                true
            } else {
                false
            }
        }
        _ => false,
    }
}
