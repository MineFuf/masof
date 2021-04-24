pub use masof::*;

use crossterm::event::Event;
use futures::StreamExt;
use futures::{select, FutureExt};
use futures_timer::Delay;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use std::io::{Stdout, stdout};
use structopt::StructOpt;
use thiserror::Error;

#[derive(StructOpt, Debug)]
pub struct Opt {
    #[structopt(name = "pathname", short = "p")]
    pub pathname: Option<PathBuf>,

    #[structopt(help = "Logging file for debugging", short = "l", long = "log-file")]
    log_file: Option<String>,

    #[structopt(
        help = "Logging level for debugging (info/debug)",
        short = "v",
        long = "log-level"
    )]
    log_level: Option<String>,

    #[structopt(help = "Demo bottom screen", short = "b", long = "bottom")]
    bottom: Option<usize>,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("CrossTerm error; {0}")]
    CrossTermError(#[from] crossterm::ErrorKind),
    #[error("Invalid logging level")]
    InvalidLoggingLevel,
    #[error("Io error; {0}")]
    IoError(#[from] std::io::Error),
    #[error("Renderer error; {0}")]
    DrawBufferError(#[from] masof::renderer::Error),
}

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
enum MainAction {
    Quit,
    Edit,
    Main,
}

#[derive(Debug)]
enum Mode {
    Main,
    Edit,
}

struct Main {
    main_mode_map: KeyMap<MainAction>,
    edit_mode_map: KeyMap<MainAction>,
    leave: bool,
    renderer: Renderer,
    start_time: Instant,
    read_line: ReadLine,
    mode: Mode,
}

impl Main {
    fn new(_opt: &Opt) -> Result<Self, Error> {
        let mut renderer = Renderer::default();
        if let Some(bottom) = _opt.bottom {
            renderer.set_bottom_screen(bottom as u16);
        }

        Ok(Self {
            leave: false,
            main_mode_map: KeyMap::new(),
            edit_mode_map: KeyMap::new(),
            renderer,
            start_time: Instant::now(),
            read_line: ReadLine::new(),
            mode: Mode::Main,
        })
    }

    fn init_key_maps(&mut self) {
        let m = &mut self.main_mode_map;
        m.add_no_mods(KeyCode::Char('q'), MainAction::Quit);
        m.add_no_mods(KeyCode::Enter, MainAction::Edit);

        let m = &mut self.edit_mode_map;
        m.add_no_mods(KeyCode::Enter, MainAction::Main);
    }

    async fn run(mut self) -> Result<(), Error> {
        self.init_key_maps();

        let mut stdout = stdout();

        self.renderer.term_on(&mut stdout)?;
        let r = self.event_loop(&mut stdout).await;
        self.renderer.term_off(&mut stdout)?;

        r
    }

    async fn event_loop(&mut self, stdout: &mut Stdout) -> Result<(), Error> {
        let mut reader = crossterm::event::EventStream::new();

        self.redraw(stdout)?;

        while !self.leave {
            let delay = Delay::new(Duration::from_millis(100));

            select! {
                _ = delay.fuse() => {  },
                maybe_event = reader.next().fuse() => {
                    if let Some(Ok(event)) = maybe_event {
                        self.renderer.event(&event);
                    }
                    match maybe_event {
                        Some(Ok(Event::Mouse{..})) => continue,
                        Some(Ok(event)) => { self.on_event(event)? }
                        Some(Err(_)) => {
                            break;
                        }
                        None => {}
                    }
                }
            }

            self.redraw(stdout)?;
        }

        Ok(())
    }

    fn main_action(&mut self, action: MainAction) -> Result<(), Error> {
        match action {
            MainAction::Quit => {
                self.leave = true;
            }
            MainAction::Edit => {
                self.mode = Mode::Edit;
            }
            MainAction::Main => {
                self.mode = Mode::Main;
            }
        }

        Ok(())
    }

    fn on_event(&mut self, event: crossterm::event::Event) -> Result<(), Error> {
        match event {
            Event::Key(event) => match self.mode {
                Mode::Main => {
                    let action = self.main_mode_map.get_action(event).map(|x| x.clone());
                    match action {
                        Some(action) => self.main_action(action)?,
                        None => {}
                    }
                }
                Mode::Edit => {
                    if let Some(action) = ReadLine::def_key_map().get_action(event) {
                        self.read_line.apply_action(action, event);
                    } else {
                        let action = self.edit_mode_map.get_action(event).map(|x| x.clone());
                        match action {
                            Some(action) => self.main_action(action)?,
                            None => {}
                        }
                    }
                }
            },
            _ => {}
        }

        Ok(())
    }

    fn redraw(&mut self, stdout: &mut Stdout) -> Result<(), Error> {
        self.renderer.begin()?;
        self.renderer.draw_str(
            10,
            1,
            &format!("masof-simple {:?}", std::time::SystemTime::now()),
            ContentStyle::default(),
        );

        self.renderer.draw_str(
            8,
            8,
            &format!("mode {:?} (hit Enter to change, q to quit)", self.mode),
            ContentStyle::default(),
        );

        for i in 0..10 {
            self.renderer.draw_str(
                0,
                i,
                &format!("{:?}", i),
                ContentStyle::default().foreground(Color::Rgb { r: 255, g: 0, b: 0 }),
            );
        }

        let time = (self.start_time.elapsed().as_millis() / 20) as u64;
        let x = (time % (self.renderer.width() as u64)) as u16;

        self.renderer.draw_str(
            2 + x,
            3,
            &"test test",
            ContentStyle::default().foreground(Color::Rgb { r: 0, g: 255, b: 0 }),
        );
        self.renderer.draw_str(
            4 + x,
            3,
            &"test",
            ContentStyle::default().foreground(Color::Rgb {
                r: 0,
                g: 255,
                b: 255,
            }),
        );

        if let Mode::Edit = self.mode {
            let l = self.renderer.height() - 1;
            self.renderer
                .draw_ansi(0, l, &ansi_term::Style::default().paint(":"));
            let pos = (1, l);
            self.read_line.draw(
                pos.0,
                pos.1,
                30,
                &mut self.renderer,
                ReadLine::def_style_map(),
            );
            self.renderer
                .set_cursor(Some((pos.0 + self.read_line.get_cursor(), pos.1)));
        } else {
            self.renderer.set_cursor(None);
        }

        self.renderer.end(stdout)?;

        Ok(())
    }
}

fn main() -> Result<(), Error> {
    let opt = Opt::from_args();
    if let (Some(log_file), Some(log_level)) = (&opt.log_file, &opt.log_level) {
        use log::LevelFilter;
        let level_filter = match log_level.as_str() {
            "debug" => LevelFilter::Debug,
            "info" => LevelFilter::Info,
            _ => return Err(Error::InvalidLoggingLevel),
        };
        if let Ok(log_file) = std::fs::File::create(log_file) {
            //
            // Find out how to flush while idle or on panic and use this:
            //
            // let log_file = std::io::BufWriter::with_capacity(0x10000, log_file);
            //
            simple_logging::log_to(log_file, level_filter);
        }
    }

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { Main::new(&opt)?.run().await })?;
    Ok(())
}
