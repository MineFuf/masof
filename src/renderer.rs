//! Double buffering terminal renderer

use std::io::stdout;
use std::io::Write;

use ansi_term::{ANSIString, ANSIStrings};
use crossterm::QueueableCommand;
use crossterm::{
    cursor,
    cursor::MoveTo,
    event::Event,
    style,
    style::{Color, ContentStyle, Print, SetAttributes, SetBackgroundColor, SetForegroundColor},
    terminal,
    terminal::{Clear, ClearType},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use thiserror::Error;
use unicode_width::UnicodeWidthChar;

#[derive(Error, Debug)]
pub enum Error {
    #[error("CrossTerm error; {0}")]
    CrossTermError(#[from] crossterm::ErrorKind),
    #[error("Invalid logging level")]
    InvalidLoggingLevel,
    #[error("Io error; {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Clone, Eq, PartialEq)]
struct CellContent {
    c: char,
    width: u8,
    style: ContentStyle,
}

impl CellContent {
    fn new(c: char, style: ContentStyle) -> Self {
        CellContent {
            c,
            width: c.width().unwrap_or(1) as u8,
            style,
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
enum Cell {
    Content(CellContent),
    WideExtension,
}

impl Cell {
    fn new(c: char, style: ContentStyle) -> Self {
        Cell::Content(CellContent::new(c, style))
    }
}

#[derive(Clone, Eq, PartialEq)]
struct VirtualBuffer {
    cells: Vec<Vec<Cell>>,
    cursor: Option<(u16, u16)>,
    width: u16,
    height: u16,
}

impl VirtualBuffer {
    fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cells: vec![vec![Cell::new(' ', ContentStyle::default())]],
            cursor: None,
        }
    }

    fn resize(&mut self, width: u16, height: u16) {
        if self.width == width && self.height == height {
            return;
        }

        self.cells.resize(height as usize, vec![]);

        for i in 0..height as usize {
            self.cells[i].resize(width as usize, Cell::new(' ', ContentStyle::default()));
        }

        self.width = width;
        self.height = height;
    }

    fn clear(&mut self) {
        self.cursor = None;

        for y in 0..self.height as usize {
            for x in 0..self.width as usize {
                self.cells[y][x] = Cell::new(' ', ContentStyle::default());
            }
        }
    }
}

pub type NrLines = u16;

pub enum Config {
    FullScreen,
    BottomScreen(NrLines, Option<(u16, u16)>),
}

pub struct Renderer {
    term_size: (u16, u16),
    config: Config,
    next: VirtualBuffer,
    prev: VirtualBuffer,
    full_refresh: bool,
}

impl Default for Renderer {
    fn default() -> Self {
        Self {
            term_size: (1, 1),
            config: Config::FullScreen,
            next: VirtualBuffer::new(1, 1),
            prev: VirtualBuffer::new(1, 1),
            full_refresh: true,
        }
    }
}

impl VirtualBuffer {
    fn putchar(&mut self, x: u16, y: u16, c: char, style: ContentStyle) -> Option<u16> {
        let c = CellContent::new(c, style);
        if c.width as usize + x as usize > self.width as usize {
            return None;
        }
        if y as usize >= self.cells.len() {
            return None;
        }

        let width = c.width;
        self.cells[y as usize][x as usize] = Cell::Content(c);

        for x in x + 1..x + width as u16 {
            self.cells[y as usize][x as usize] = Cell::WideExtension;
        }

        Some(width as u16)
    }
}

impl Renderer {
    pub fn bottom_screen(mut self, min_nr_lines: u16) -> Self {
        self.set_bottom_screen(min_nr_lines);
        self
    }

    pub fn set_bottom_screen(&mut self, min_nr_lines: u16) -> &mut Self {
        self.config = Config::BottomScreen(min_nr_lines, None);
        self
    }

    pub fn width(&self) -> u16 {
        self.term_size.0
    }

    pub fn height(&self) -> u16 {
        match &self.config {
            Config::FullScreen => self.term_size.1,
            Config::BottomScreen(lines, _) => std::cmp::min(*lines, self.term_size.1),
        }
    }

    pub fn term_on(&mut self) -> Result<(), Error> {
        let mut stdout = stdout();

        terminal::enable_raw_mode()?;
        stdout.execute(cursor::Hide)?;

        let (x, y) = crossterm::terminal::size()?;
        self.on_resize(x, y);

        match &mut self.config {
            Config::FullScreen => {
                stdout.execute(EnterAlternateScreen)?;
            }
            Config::BottomScreen(lines, pos) => {
                // Make space for new lines
                let l = std::cmp::min(*lines, self.term_size.1);
                let position = crossterm::cursor::position()?;
                let y = std::cmp::min(self.term_size.1 - l, position.1);
                for yl in 0..l {
                    if yl + 1 >= l && y != position.1 {
                        break;
                    }

                    stdout.execute(style::ResetColor)?;
                    stdout.execute(Print("\n"))?;
                    stdout.execute(Clear(ClearType::UntilNewLine))?;
                }
                *pos = Some(position);
            }
        };

        Ok(())
    }

    pub fn term_off(&mut self) -> Result<(), Error> {
        let mut stdout = stdout();

        match self.config {
            Config::FullScreen => {
                stdout.execute(LeaveAlternateScreen)?;
            }
            Config::BottomScreen(lines, position) => {
                // Clear lines
                let position = position.clone().take().unwrap_or((0, 0));
                let l = std::cmp::min(lines, self.term_size.1);
                let y = std::cmp::min(self.term_size.1 - l, position.1);
                stdout.execute(MoveTo(position.0, y))?;
                for yl in 0..l {
                    stdout.execute(style::ResetColor)?;
                    stdout.execute(Clear(ClearType::UntilNewLine))?;
                    if yl + 1 >= l && y != position.1 {
                        break;
                    }
                    stdout.execute(Print("\n"))?;
                }
                stdout.execute(MoveTo(position.0, y))?;
            }
        };

        stdout.execute(cursor::Show)?;
        terminal::disable_raw_mode()?;

        Ok(())
    }

    fn on_resize(&mut self, x: u16, y: u16) {
        let prev_term_size = self.term_size;
        self.term_size = (x, y);

        let y = match &mut self.config {
            Config::FullScreen => y,
            Config::BottomScreen(lines, position) => {
                match position {
                    None => {}
                    Some(position) => {
                        let l = std::cmp::min(*lines, prev_term_size.1);
                        let y = std::cmp::min(prev_term_size.1 - l, position.1);
                        if y != position.1 {
                            position.1 += self.term_size.1;
                            position.1 -= prev_term_size.1;
                        }
                    }
                }

                std::cmp::min(*lines, y)
            }
        };

        self.next.resize(x, y);
        self.prev.resize(x, y);
        self.full_refresh = true;
    }

    pub fn event(&mut self, event: &Event) {
        match event {
            Event::Resize(x, y) => {
                self.on_resize(*x, *y);
            }
            _ => {}
        }
    }

    pub fn draw_str(&mut self, mut x: u16, y: u16, s: &str, style: ContentStyle) -> u16 {
        let start_x = x;
        for c in s.chars() {
            if let Some(w) = self.next.putchar(x, y, c, style) {
                x += w;
            } else {
                break;
            }
        }

        x - start_x
    }

    pub fn draw_ansi<'a>(&mut self, x: u16, y: u16, s: &ANSIString<'a>) -> u16 {
        let style = s.style_ref();

        use ansi_term::Colour;
        fn convert_color(color: Colour) -> Color {
            match color {
                Colour::Black => Color::Black,
                Colour::Red => Color::Red,
                Colour::Green => Color::Green,
                Colour::Yellow => Color::Yellow,
                Colour::Blue => Color::Blue,
                Colour::Purple => Color::Magenta,
                Colour::Cyan => Color::Cyan,
                Colour::White => Color::White,
                Colour::Fixed(v) => Color::AnsiValue(v),
                Colour::RGB(r, g, b) => Color::Rgb { r, g, b },
            }
        }

        let content_style = ContentStyle {
            background_color: style.background.map(convert_color),
            foreground_color: style.foreground.map(convert_color),
            attributes: {
                let attr = crossterm::style::Attributes::default();

                attr
            },
        };

        self.draw_str(x, y, &*s, content_style)
    }

    pub fn draw_ansis<'a>(&mut self, mut x: u16, y: u16, s: ANSIStrings<'a>) -> u16 {
        let start_x = x;

        for i in s.0.iter() {
            x += self.draw_ansi(x, y, i);
        }

        x - start_x
    }

    pub fn set_cursor(&mut self, info: Option<(u16, u16)>) {
        self.next.cursor = info;
    }

    pub fn begin(&mut self) -> Result<(), Error> {
        self.next.clear();
        Ok(())
    }

    pub fn end(&mut self) -> Result<(), Error> {
        let top_left = match self.config {
            Config::FullScreen => (0, 0),
            Config::BottomScreen(lines, position) => {
                let position = position.clone().take().unwrap_or((0, 0));
                let l = std::cmp::min(lines, self.term_size.1);
                let y = std::cmp::min(self.term_size.1 - l, position.1);
                (0, y)
            }
        };

        let mut stdout = stdout();
        let next = &self.next;
        let prev = &self.prev;
        let mut style = ContentStyle::default();

        for y in 0..next.height as usize {
            if next.cells[y] == prev.cells[y] && !self.full_refresh {
                // Skip unmodified lines.
                continue;
            }

            stdout.queue(MoveTo(0, top_left.1 + y as u16))?;

            // TODO: find a subrange that is modified and keep the rest of the line as
            // it is.
            for x in 0..next.width as usize {
                match &next.cells[y][x] {
                    Cell::Content(content) => {
                        if style != content.style {
                            if style.background_color != content.style.background_color {
                                match content.style.background_color {
                                    Some(x) => {
                                        stdout.queue(SetBackgroundColor(x))?;
                                    }
                                    None => {
                                        stdout.queue(SetBackgroundColor(Color::Reset))?;
                                    }
                                }
                            }
                            if style.foreground_color != content.style.foreground_color {
                                match content.style.foreground_color {
                                    Some(x) => {
                                        stdout.queue(SetForegroundColor(x))?;
                                    }
                                    None => {
                                        stdout.queue(SetForegroundColor(Color::Reset))?;
                                    }
                                }
                            }
                            if style.attributes != content.style.attributes {
                                stdout.queue(SetAttributes(content.style.attributes))?;
                            }
                            style = content.style;
                        }
                        stdout.queue(Print(content.c))?;
                    }
                    _ => {}
                }
            }
        }

        if let Some(position) = next.cursor {
            stdout.execute(MoveTo(position.0 + top_left.0, position.1 + top_left.1))?;
            stdout.execute(cursor::Show)?;
        } else {
            stdout.execute(cursor::Hide)?;
        }

        stdout.flush()?;
        self.full_refresh = false;

        std::mem::swap(&mut self.next, &mut self.prev);
        Ok(())
    }
}
