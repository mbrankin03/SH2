//! Dialogue script table parser for Silent Hill 2 `.mes` files.
//!
//! # Format Architecture
//! Silent Hill 2 message files (`data/etc/message/*.mes`) contain localized strings, dialogue
//! subtitles, and interactive prompts.
//!
//! ### Binary Layout:
//! - `u16 count`: Total message count.
//! - `[u16; count] offsets`: Offset of each message in 16-bit words (multiply by 2 for byte offset).
//!
//! ### Message Content & Encoding:
//! - Messages are partitioned into one or more pages separated by `0xFF, 0xFF`.
//! - Page boundaries align to 16-bit boundaries with style control words (e.g. `0x1000`, `0x8000`, `0x9000`).
//! - Character encoding:
//!   - Byte `b < 0xFF`: character is `(b + 0x20) as char` (e.g., `0x00` -> `' '` space, `0x21` -> `'A'`, etc.).
//!     Bytes `>= 0x60` fall back to `?` unless extended.
//!   - Byte `0xFF`: Escape sequence marker:
//!     - `0xFF, 0xFF`: End of page (join pages with `\x0C` form feed).
//!     - `0xFF, 0xFD`: Newline `\n`.
//!     - `0xFF, 0xE0..=0xE3`: Variable substitution marker (e.g., dynamic numbers or item names).
//!     - `0xFF, 0xF6`: Choice prompt follows (e.g. Yes/No question).
//!     - Other escape codes represent styling or color tags and are skipped.

use crate::error::AssetError;

/// A parsed dialogue message consisting of one or more formatted pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogueMessage {
    /// Zero-based message index in the `.mes` table.
    pub id: usize,
    /// Pages of text contained in this message.
    pub pages: Vec<String>,
    /// Full text with pages joined by form-feed (`\x0C`).
    pub text: String,
}

/// Collection of dialogue messages parsed from a `.mes` file.
#[derive(Debug, Clone, Default)]
pub struct MessageTable {
    /// Sequential list of dialogue messages.
    pub messages: Vec<DialogueMessage>,
}

impl MessageTable {
    /// Retrieve a message by index.
    pub fn get(&self, index: usize) -> Option<&DialogueMessage> {
        self.messages.get(index)
    }

    /// Retrieve the formatted text for a message by index, if present.
    pub fn get_text(&self, index: usize) -> Option<&str> {
        self.messages.get(index).map(|m| m.text.as_str())
    }
}

/// Parses a `.mes` dialogue script file from binary data.
pub fn parse_mes(data: &[u8]) -> Result<MessageTable, AssetError> {
    if data.len() < 2 {
        return Err(AssetError::BufferUnderflow {
            required: 2,
            available: data.len(),
        });
    }

    let count = u16::from_le_bytes([data[0], data[1]]) as usize;
    let table_header_size = 2 + count * 2;
    if data.len() < table_header_size {
        return Err(AssetError::BufferUnderflow {
            required: table_header_size,
            available: data.len(),
        });
    }

    let mut offsets = Vec::with_capacity(count);
    for i in 0..count {
        let ofs = 2 + i * 2;
        let word_offset = u16::from_le_bytes([data[ofs], data[ofs + 1]]) as usize;
        offsets.push(word_offset * 2);
    }

    let mut messages = Vec::with_capacity(count);

    for (i, &start) in offsets.iter().enumerate() {
        let end = if i + 1 < count {
            offsets[i + 1].min(data.len())
        } else {
            data.len()
        };

        if start >= data.len() || start >= end {
            messages.push(DialogueMessage {
                id: i,
                pages: Vec::new(),
                text: String::new(),
            });
            continue;
        }

        let mut p = start;
        let mut pages = Vec::new();
        let mut cur_page = String::new();

        while p < end {
            if cur_page.is_empty() {
                // Align to 16-bit boundary if needed
                if p % 2 != 0 {
                    p += 1;
                }
                // Skip style control words (low byte 0, high byte & 0x0F == 0)
                while p + 1 < end && data[p] == 0 && data[p + 1] != 0 && (data[p + 1] & 0x0F) == 0 {
                    p += 2;
                }
                if p >= end {
                    break;
                }
            }

            let b = data[p];
            if b == 0xFF && p + 1 < end {
                let code = data[p + 1];
                p += 2;
                match code {
                    0xFF => {
                        let trimmed = cur_page.trim().to_string();
                        if !trimmed.is_empty() {
                            pages.push(trimmed);
                        }
                        cur_page.clear();
                    }
                    0xFD => {
                        cur_page.push('\n');
                    }
                    0xE0..=0xE3 => {
                        // Variable insertion parameter code
                        cur_page.push((1 + code - 0xE0) as char);
                    }
                    0xF6 => {
                        // Yes/No choice prompt indicator
                        cur_page.push('\x05');
                    }
                    _ => {
                        // Miscellaneous style/formatting escape codes: drop
                    }
                }
                continue;
            }

            if b < 0x60 {
                cur_page.push((b + 0x20) as char);
            } else if b < 0xFF {
                cur_page.push('?');
            }
            p += 1;
        }

        let trimmed = cur_page.trim().to_string();
        if !trimmed.is_empty() {
            pages.push(trimmed);
        }

        let text = pages.join("\x0C");
        messages.push(DialogueMessage { id: i, pages, text });
    }

    Ok(MessageTable { messages })
}
