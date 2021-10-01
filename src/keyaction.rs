//! Types to manage mapping of key combinations to actions

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;
use std::fmt::Write;

#[derive(Debug, Hash, Copy, Clone, Default, Eq, PartialEq)]
pub struct Modifiers {
    ctrl: bool,
    alt: bool,
    shift: bool,
}

impl Modifiers {
    fn shift(self) -> Self {
        Self {
            shift: true,
            ..self
        }
    }

    fn ctrl(self) -> Self {
        Self { ctrl: true, ..self }
    }
}

#[derive(Eq, Hash, PartialEq, Debug)]
pub enum KeyCombination {
    Specific(KeyCode, Modifiers),
    AllChars(Modifiers),
}

use std::fmt;

impl fmt::Display for KeyCombination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyCombination::Specific(key_code, modifiers) => {
                if modifiers.ctrl {
                    write!(f, "C-")?;
                }
                if modifiers.alt {
                    write!(f, "M-")?;
                }
                if modifiers.shift {
                    write!(f, "S-")?;
                }
                let s = match key_code {
                    KeyCode::Backspace => "Backspace".to_owned(),
                    KeyCode::Enter => "Enter".to_owned(),
                    KeyCode::Left => "Left".to_owned(),
                    KeyCode::Right => "Right".to_owned(),
                    KeyCode::Up => "Up".to_owned(),
                    KeyCode::Down => "Down".to_owned(),
                    KeyCode::Home => "Home".to_owned(),
                    KeyCode::End => "End".to_owned(),
                    KeyCode::PageUp => "PageUp".to_owned(),
                    KeyCode::PageDown => "PageDown".to_owned(),
                    KeyCode::Tab => "Tab".to_owned(),
                    KeyCode::BackTab => "BackTab".to_owned(),
                    KeyCode::Delete => "Delete".to_owned(),
                    KeyCode::Insert => "Insert".to_owned(),
                    KeyCode::F(i) => format!("F{}", i),
                    KeyCode::Char(' ') => format!("Space"),
                    KeyCode::Char('*') => format!("'*'"),
                    KeyCode::Char(',') => format!("','"),
                    KeyCode::Char(ch) => format!("{}", ch),
                    KeyCode::Null => format!("<null>"),
                    KeyCode::Esc => format!("Esc"),
                };
                write!(f, "{}", s)
            }
            _ => write!(f, "?"),
        }
    }
}

pub struct KeyMap<A> {
    map: HashMap<KeyCombination, A>,
}

impl<A> Default for KeyMap<A> {
    fn default() -> Self {
        KeyMap::new()
    }
}

impl<A> KeyMap<A> {
    pub fn new() -> Self {
        Self {
            map: Default::default(),
        }
    }

    pub fn map(&self) -> &HashMap<KeyCombination, A> {
        &self.map
    }

    pub fn add_no_mods(&mut self, code: KeyCode, a: A) {
        self.map
            .insert(KeyCombination::Specific(code, Modifiers::default()), a);
    }

    pub fn add_ctrl(&mut self, code: KeyCode, a: A) {
        self.map.insert(
            KeyCombination::Specific(code, Modifiers::default().ctrl()),
            a,
        );
    }

    pub fn add_shift(&mut self, code: KeyCode, a: A) {
        self.map.insert(
            KeyCombination::Specific(code, Modifiers::default().shift()),
            a,
        );
    }

    pub fn add_char_no_handler(&mut self, a: A) {
        self.map
            .insert(KeyCombination::AllChars(Modifiers::default()), a);
    }

    pub fn add_char_shift(&mut self, a: A) {
        self.map
            .insert(KeyCombination::AllChars(Modifiers::default().shift()), a);
    }

    pub fn get_action(&self, key_event: KeyEvent) -> Option<&A> {
        let modifiers = key_event.modifiers;
        let modifiers = Modifiers {
            ctrl: modifiers.contains(KeyModifiers::CONTROL),
            shift: modifiers.contains(KeyModifiers::SHIFT),
            alt: modifiers.contains(KeyModifiers::ALT),
        };
        if let Some(action) = self
            .map
            .get(&KeyCombination::Specific(key_event.code, modifiers))
        {
            return Some(action);
        }
        if let KeyCode::Char(_) = key_event.code {
            if let Some(action) = self.map.get(&KeyCombination::AllChars(modifiers)) {
                return Some(action);
            }
        }
        None
    }

    pub fn describe(&self, output: &mut String)
        where A: std::fmt::Display + Ord
    {
        let mut action_to_keys = std::collections::BTreeMap::new();
        for (key, value) in self.map.iter() {
            use std::collections::btree_map;
            let v = match action_to_keys.entry(value) {
                btree_map::Entry::Vacant(v) => v.insert(vec![]),
                btree_map::Entry::Occupied(o) => o.into_mut(),
            };
            v.push(key);
        }

        for (action, mut keys) in action_to_keys.into_iter() {
            let mut str_keys = vec![];
            for key in keys.drain(..) {
                match key {
                    KeyCombination::Specific(_, _) => {
                        str_keys.push(format!("{}", key));
                    }
                    KeyCombination::AllChars { .. } => {
                        if str_keys.len() == 0 {
                            str_keys.push("<char>".to_string());
                        } else {
                        }
                    }
                }
            }
            let _ = writeln!(
                output,
                "    {:width$}  - {}",
                str_keys.join(" / "),
                action,
                width = 17
            );
        }
        let _ = writeln!(output, "");
    }
}

pub enum TreeNode<A> {
    Tree(KeyTree<A>),
    Action(A),
}

impl<A> Default for TreeNode<A>
where
    A: Default,
{
    fn default() -> Self {
        TreeNode::Action(Default::default())
    }
}

#[derive(Default)]
pub struct KeyTree<A> {
    map: KeyMap<TreeNode<A>>,
}

impl<A> KeyTree<A> {
    pub fn new() -> Self {
        Self { map: KeyMap::new() }
    }

    pub fn map(&self) -> &KeyMap<TreeNode<A>> {
        &self.map
    }

    pub fn add_vector(&mut self, _code: Vec<KeyCombination>, _a: A) {}

    pub fn add_no_mods(&mut self, code: KeyCode, a: A) {
        self.map.add_no_mods(code, TreeNode::Action(a))
    }

    pub fn add_ctrl(&mut self, code: KeyCode, a: A) {
        self.map.add_ctrl(code, TreeNode::Action(a))
    }

    pub fn add_shift(&mut self, code: KeyCode, a: A) {
        self.map.add_shift(code, TreeNode::Action(a))
    }

    pub fn add_char_no_handler(&mut self, a: A) {
        self.map.add_char_no_handler(TreeNode::Action(a))
    }

    pub fn add_char_shift(&mut self, a: A) {
        self.map.add_char_shift(TreeNode::Action(a))
    }
}
