//! Dialogue script table parser for Silent Hill 2 `.mes` files.
//!
//! # Format Architecture (`SLUS_202.28` `Font/font.c` `fontPrintStrMain` `0x001b4a10`)
//! Silent Hill 2 message files (`data/etc/message/*.mes`) contain localized strings, dialogue
//! subtitles, and interactive prompts.
//!
//! ### Binary Layout:
//! - `u16 count`: Total message count.
//! - `[u16; count] offsets`: Offset of each message in 16-bit words (multiply by 2 for byte offset).
//!
//! ### Message Content & Encoding (`0x001b4a10`, lines 745–761):
//! - Each page begins on a 16-bit word boundary. Word `0x8000` switches the reader into
//!   byte-packed mode (used by all English retail `.mes` files).
//! - In byte-packed mode:
//!   - `0x00..=0x5F`: ASCII character `(b + 0x20) as char` (`0x00` -> `' '`, `0x21` -> `'A'`).
//!   - `0x60..=0xDF`: Extended single-byte glyph codes (`0x8E` -> `'®'` for `DUALSHOCK®2`).
//!   - `0xE0, lo, hi`: 3-byte extended 16-bit glyph code (`0x001b4a10:752-755`).
//!   - `0xE1..=0xFE, arg`: 2-byte layout/kerning control codes (`0xF800..0xF9FF` Y pos,
//!     `0xFA00..0xFBFF` X pos, `0xFC00..0xFCFF` neg X, `0xFD00..0xFDFF` pos X,
//!     `0xFE00..0xFEFF` horizontal rule; `0x001b4a10:756-759`).
//!   - `0xFF, code`: 2-byte control codes:
//!     - `0xFF, 0xFF`: End of page, followed by 16-bit alignment and a 16-bit `end_ctrl` word
//!       (`fontNextMessage` `0x001b9cb0`; bit 15 `0x8000` marks the final page).
//!     - `0xFF, 0xFD`: Newline `'\n'`.
//!     - `0xFF, 0xFE`: Full-width space `' '`.
//!     - `0xFF, 0x00..=0x63` / `0xE0..=0xE9` / `0xF5..=0xFC`: Color, variable, and prompt tags.

use crate::error::AssetError;

/// A parsed dialogue message consisting of one or more formatted pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogueMessage {
    /// Zero-based message index in the `.mes` table.
    pub id: usize,
    /// Pages of plain text contained in this message.
    pub pages: Vec<String>,
    /// Pages with inline `fontSetColor` (`0x001b73a0`) palette indices `(color_idx, text)` preserved
    /// (e.g., `0` = white, `3` = green item-of-interest highlight from `0xFF 0x03`).
    pub colored_pages: Vec<Vec<(u8, String)>>,
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
                colored_pages: Vec::new(),
                text: String::new(),
            });
            continue;
        }

        let mut p = start;
        let mut pages = Vec::new();
        let mut colored_pages: Vec<Vec<(u8, String)>> = Vec::new();
        let mut cur_page = String::new();
        let mut cur_spans: Vec<(u8, String)> = Vec::new();
        let mut cur_color: u8 = 0;
        let mut byte_packed = false;
        let mut at_page_start = true;

        let push_ch =
            |cur_page: &mut String, cur_spans: &mut Vec<(u8, String)>, color: u8, ch: char| {
                cur_page.push(ch);
                if let Some((last_col, last_str)) = cur_spans.last_mut() {
                    if *last_col == color {
                        last_str.push(ch);
                        return;
                    }
                }
                let mut s = String::new();
                s.push(ch);
                cur_spans.push((color, s));
            };

        let flush_page = |pages: &mut Vec<String>,
                          colored_pages: &mut Vec<Vec<(u8, String)>>,
                          cur_page: &mut String,
                          cur_spans: &mut Vec<(u8, String)>,
                          cur_color: &mut u8| {
            let trimmed = cur_page.trim().to_string();
            if !trimmed.is_empty() {
                pages.push(trimmed);
                colored_pages.push(std::mem::take(cur_spans));
            } else {
                cur_spans.clear();
            }
            cur_page.clear();
            *cur_color = 0;
        };

        while p < end {
            if at_page_start {
                // Align to 16-bit word boundary at the start of every page (`fontPrintStrMain` `0x001b4a10`).
                if p % 2 != 0 {
                    p += 1;
                }
                if p + 1 >= end {
                    break;
                }
                byte_packed = false;
                at_page_start = false;
            }

            if !byte_packed {
                if p + 1 >= end {
                    break;
                }
                let word = u16::from_le_bytes([data[p], data[p + 1]]);
                p += 2;
                if word == 0x8000 {
                    // Enter byte-packed stream mode (`fontPrintStrMain` `0x001b4a10:740-742`).
                    byte_packed = true;
                    continue;
                }
                if word == 0xFFFF {
                    flush_page(
                        &mut pages,
                        &mut colored_pages,
                        &mut cur_page,
                        &mut cur_spans,
                        &mut cur_color,
                    );
                    at_page_start = true;
                    // Consume trailing 16-bit `end_ctrl` word (`fontNextMessage` `0x001b9cb0`).
                    if p + 1 < end {
                        let end_ctrl = u16::from_le_bytes([data[p], data[p + 1]]);
                        p += 2;
                        if (end_ctrl & 0x8000) != 0 {
                            break;
                        }
                    } else {
                        break;
                    }
                    continue;
                }
                match word {
                    0xFFFD => push_ch(&mut cur_page, &mut cur_spans, cur_color, '\n'),
                    0xFFFE => push_ch(&mut cur_page, &mut cur_spans, cur_color, ' '),
                    0xFF00..=0xFF10 => cur_color = (word & 0xFF) as u8,
                    0x0000..=0x005F => {
                        push_ch(
                            &mut cur_page,
                            &mut cur_spans,
                            cur_color,
                            ((word as u8) + 0x20) as char,
                        );
                    }
                    0x008E => push_ch(&mut cur_page, &mut cur_spans, cur_color, '®'),
                    // Other control words (`0xF800..=0xFFFC`) and extended non-ASCII glyphs: omit cleanly.
                    _ => {}
                }
                continue;
            }

            // Byte-packed stream mode (`fontPrintStrMain` `0x001b4a10:749-761`).
            let b = data[p];
            p += 1;
            if b == 0xE0 {
                // 3-byte extended 16-bit glyph code (`0xE0, lo, hi`; `0x001b4a10:752-755`).
                p = (p + 2).min(end);
                continue;
            }
            if b > 0xE0 {
                // 2-byte control code `((b << 8) | arg)` (`0x001b4a10:756-759`).
                if p >= end {
                    break;
                }
                let arg = data[p];
                p += 1;
                if b == 0xFF {
                    match arg {
                        0xFF => {
                            flush_page(
                                &mut pages,
                                &mut colored_pages,
                                &mut cur_page,
                                &mut cur_spans,
                                &mut cur_color,
                            );
                            // Align to 16-bit boundary and consume `end_ctrl` (`0x001b9cb0`).
                            if p % 2 != 0 {
                                p += 1;
                            }
                            at_page_start = true;
                            if p + 1 < end {
                                let end_ctrl = u16::from_le_bytes([data[p], data[p + 1]]);
                                p += 2;
                                if (end_ctrl & 0x8000) != 0 {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                        0xFD => push_ch(&mut cur_page, &mut cur_spans, cur_color, '\n'),
                        0xFE => push_ch(&mut cur_page, &mut cur_spans, cur_color, ' '),
                        0x00..=0x10 => cur_color = arg,
                        // Variable slots (`0xE0..=0xE9`), Yes/No & layout tags (`0xF5..=0xFC`).
                        _ => {}
                    }
                }
                // `0xE1..=0xFE` (`0xF8xx` Y pos, `0xFAxx` X pos, `0xFCxx`/`0xFDxx` kern, `0xFExx` rule): drop cleanly.
                continue;
            }

            if b < 0x60 {
                push_ch(&mut cur_page, &mut cur_spans, cur_color, (b + 0x20) as char);
            } else if b == 0x8E {
                push_ch(&mut cur_page, &mut cur_spans, cur_color, '®');
            }
        }

        flush_page(
            &mut pages,
            &mut colored_pages,
            &mut cur_page,
            &mut cur_spans,
            &mut cur_color,
        );

        let text = pages.join("\x0C");
        messages.push(DialogueMessage {
            id: i,
            pages,
            colored_pages,
            text,
        });
    }

    Ok(MessageTable { messages })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mes_byte_packed_control_codes_no_question_mark_leaks() {
        // Synthetic 1-message `.mes`:
        // header: count=1 (0x0001), offset=2 words (4 bytes)
        // msg 0: 0x8000 (byte-packed), 0xF8 0x20 (Y pos), 0xFA 0x10 (X pos),
        //        0xFC 0x04 (neg kern), 0xFD 0x02 (pos kern), 0xFE 0x40 (rule),
        //        0xE0 0x82 0x01 (3-byte extended glyph),
        //        0xFF 0x03 (green color), "Hi" (0x28, 0x49), 0xFF 0x00 (white),
        //        0xFF 0xFD (newline), "DUALSHOCK" + 0x8E + "2",
        //        0xFF 0xFF (end page 1), pad 0x00, 0x1000 (wait button, non-final),
        //        0x8000, "Page 2.", 0xFF 0xFF, pad 0x00, 0x9000 (wait button, final)
        let mut raw = Vec::new();
        raw.extend_from_slice(&1u16.to_le_bytes());
        raw.extend_from_slice(&2u16.to_le_bytes());
        raw.extend_from_slice(&0x8000u16.to_le_bytes());
        raw.extend_from_slice(&[0xF8, 0x20, 0xFA, 0x10, 0xFC, 0x04, 0xFD, 0x02, 0xFE, 0x40]);
        raw.extend_from_slice(&[0xE0, 0x82, 0x01]);
        raw.extend_from_slice(&[0xFF, 0x03, b'H' - 0x20, b'i' - 0x20, 0xFF, 0x00]);
        raw.extend_from_slice(&[0xFF, 0xFD]);
        for &c in b"DUALSHOCK" {
            raw.push(c - 0x20);
        }
        raw.push(0x8E);
        raw.push(b'2' - 0x20);
        raw.extend_from_slice(&[0xFF, 0xFF]);
        if raw.len() % 2 != 0 {
            raw.push(0x00);
        }
        raw.extend_from_slice(&0x1000u16.to_le_bytes());
        raw.extend_from_slice(&0x8000u16.to_le_bytes());
        for &c in b"Page 2." {
            raw.push(c - 0x20);
        }
        raw.extend_from_slice(&[0xFF, 0xFF]);
        if raw.len() % 2 != 0 {
            raw.push(0x00);
        }
        raw.extend_from_slice(&0x9000u16.to_le_bytes());

        let tbl = parse_mes(&raw).expect("parse synthetic .mes");
        assert_eq!(tbl.messages.len(), 1);
        let msg = &tbl.messages[0];
        assert_eq!(msg.pages, vec!["Hi\nDUALSHOCK®2", "Page 2."]);
        assert_eq!(
            msg.colored_pages[0],
            vec![(3, "Hi".to_string()), (0, "\nDUALSHOCK®2".to_string())]
        );
        assert!(!msg.text.contains('?'));
    }
}
