//! Single line editor widget

use super::{KeyCode, KeyEvent};

pub enum Action {
    BackDeleteChar,
    DeleteChar,
    LeftChar,
    LeftWord,
    RightChar,
    RightWord,
    DelBackWord,
    GotoLineStart,
    GotoLineEnd,
    InsertChar,
    Complete,
}

pub struct ReadLine {
    /// Cursor position
    cursor: u16,
    h_scroll: u16,
    strval: String,
}

pub struct StyleMap {
    pub main: ansi_term::Style,
    pub overflow: ansi_term::Style,
}

pub type KeyMap = super::keyaction::KeyMap<Action>;

lazy_static::lazy_static! {
    static ref DEF_STYLE_MAP : StyleMap = {
        StyleMap {
            main: Default::default(),
            overflow: Default::default(),
        }
    };

    static ref DEF_KEY_MAP : KeyMap = {
        let mut m : KeyMap = KeyMap::new();

        m.add_no_mods(KeyCode::Backspace, Action::BackDeleteChar);
        m.add_no_mods(KeyCode::Delete, Action::DeleteChar);
        m.add_no_mods(KeyCode::Left, Action::LeftChar);
        m.add_no_mods(KeyCode::Right, Action::RightChar);
        m.add_ctrl(KeyCode::Left, Action::LeftWord);
        m.add_ctrl(KeyCode::Right, Action::RightWord);
        m.add_ctrl(KeyCode::Char('w'), Action::DelBackWord);
        m.add_no_mods(KeyCode::Home, Action::GotoLineStart);
        m.add_no_mods(KeyCode::End, Action::GotoLineEnd);
        m.add_char_no_handler(Action::InsertChar);
        m.add_char_shift(Action::InsertChar);
        m.add_no_mods(KeyCode::Tab, Action::Complete);

        m
    };
}

impl ReadLine {
    pub fn def_style_map() -> &'static StyleMap {
        &*DEF_STYLE_MAP
    }

    pub fn def_key_map() -> &'static KeyMap {
        &*DEF_KEY_MAP
    }

    pub fn new() -> Self {
        Self {
            cursor: 0,
            h_scroll: 0,
            strval: Default::default(),
        }
    }

    pub fn strval(&self) -> &str {
        &self.strval
    }

    pub fn draw(
        &self,
        x: u16,
        y: u16,
        _length: u16,
        renderer: &mut super::Renderer,
        style_map: &StyleMap,
    ) {
        use ansi_term::ANSIStrings;
        let mut v = vec![];

        v.push(style_map.main.paint(&self.strval));

        renderer.draw(x, y, ANSIStrings(v.as_slice()));
    }

    pub fn get_cursor(&self) -> u16 {
        self.cursor - self.h_scroll
    }

    fn cursor(&self) -> usize {
        std::cmp::min(self.cursor as usize, self.strval.len())
    }

    pub fn apply_action(&mut self, action: &Action, event: KeyEvent) {
        match action {
            Action::InsertChar => {
                if let KeyCode::Char(c) = event.code {
                    let cursor = self.cursor();
                    self.strval =
                        format!("{}{}{}", &self.strval[..cursor], c, &self.strval[cursor..]);
                    self.cursor += 1;
                }
            }
            Action::BackDeleteChar => {
                let cursor = self.cursor();
                if cursor > 0 {
                    self.strval =
                        format!("{}{}", &self.strval[..cursor - 1], &self.strval[cursor..]);
                    self.cursor = (cursor - 1) as u16;
                }
            }
            Action::DeleteChar => {
                let cursor = self.cursor();
                if cursor < self.strval.len() {
                    self.strval =
                        format!("{}{}", &self.strval[..cursor], &self.strval[cursor + 1..]);
                    self.cursor = self.cursor() as u16;
                }
            }
            Action::LeftChar => {
                let cursor = self.cursor();
                if cursor > 0 {
                    self.cursor = (cursor - 1) as u16;
                }
            }
            Action::LeftWord => {
                if let Some(cursor) = self.left_word_offset() {
                    self.cursor = cursor as u16;
                }
            }
            Action::RightWord => {
                if let Some(cursor) = self.right_word_offset() {
                    self.cursor = cursor as u16;
                }
            }
            Action::DelBackWord => {
                let cur_cursor = self.cursor();
                if let Some(cursor) = self.left_word_offset() {
                    self.strval =
                        format!("{}{}", &self.strval[..cursor], &self.strval[cur_cursor..]);
                    self.cursor = cursor as u16;
                }
            }
            Action::GotoLineStart => {
                self.cursor = 0;
            }
            Action::GotoLineEnd => {
                self.cursor = self.strval.len() as u16;
            }
            Action::RightChar => {
                self.cursor = (self.cursor() + 1) as u16;
                self.cursor = self.cursor() as u16;
            }
            Action::Complete => {}
        }
    }

    fn left_word_offset(&self) -> Option<usize> {
        let v: Vec<_> = self.strval.chars().collect();
        let cursor = self.cursor();
        if cursor > 0 {
            let mut cursor = cursor - 1;
            while cursor > 0 {
                if v[cursor] == ' ' {
                    cursor -= 1;
                } else {
                    break;
                }
            }
            let mut prev_cursor = cursor;
            loop {
                if cursor < v.len() && v[cursor] != ' ' {
                    prev_cursor = cursor;
                    if cursor == 0 {
                        break;
                    }
                    cursor -= 1;
                } else {
                    break;
                }
            }
            return Some(prev_cursor);
        }

        None
    }

    fn right_word_offset(&self) -> Option<usize> {
        let v: Vec<_> = self.strval.chars().collect();
        let cursor = self.cursor();
        if cursor < v.len() {
            let mut cursor = cursor;
            while cursor < v.len() {
                if v[cursor] != ' ' {
                    cursor += 1;
                } else {
                    break;
                }
            }
            while cursor < v.len() {
                if v[cursor] == ' ' {
                    cursor += 1;
                } else {
                    break;
                }
            }
            return Some(cursor);
        }

        None
    }
}
