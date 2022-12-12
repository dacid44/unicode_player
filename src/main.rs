mod renderers;
mod source;
mod tui;
mod youtube;
mod terminal;

use std::collections::VecDeque;
use std::io::prelude::*;
use std::process::{Command, Stdio};
use iter_read::IterRead;
use colored::{Colorize, ColoredString};
use image::io::Reader as ImageReader;
use std::io::{Cursor, stdin, stdout};
use std::path::Path;
use image::{Pixel, RgbImage};
use termion::event::{Event, Key};
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use std::thread;
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use clap::lazy_static::lazy_static;
use clap::Parser;

use renderers::Renderer;
use crate::source::Source;
use crate::tui::{EventResponse, Tui};
use crate::terminal::{TermEvent, Terminal, TermKind, TermUtility};

lazy_static! {
    static ref EVENT_THREAD_ACCEPT_EXIT: Mutex<bool> = Mutex::new(true);
}

#[derive(Parser)]
struct Cli {
    // #[clap(validator = file_exists)]
    filename: Option<String>,
    #[clap(short, long, default_value_t = 30)]
    framerate: u32,
    #[clap(short = 'h', long, default_value_t = 2.2)]
    char_height: f32,
    #[clap(short, long, arg_enum, default_value_t = Renderer::PixelChar)]
    mode: Renderer,
    // #[clap(short, long)]
    // output: Option<String>,
}

fn file_exists(filename: &str) -> Result<(), String> {
    if std::path::Path::new(filename).is_file() {
        return Ok(());
    } else {
        return Err("Invalid input file.".to_string());
    }
}

fn main() {
    let cli = Cli::parse();

    let mut source = Source::new(cli.filename.as_deref(), cli.framerate).unwrap();

    let mut terminal = Terminal::new_crossterm();

    let (tx, rx) = channel();
    let evt_thread = thread::Builder::new()
        .name("event".to_string())
        .spawn({
            let terminal = terminal.utility();
            move || event_thread(terminal, tx)
        })
        .unwrap();

    let mut renderer = cli.mode;
    let mut tui = Tui::new(renderer.clone(), cli.char_height);

    let mut frame_times = VecDeque::from([Duration::new(0, 0); 300]);

    'frame_loop: loop {
        for event in rx.try_iter() {
            match tui.handle_event(event) {
                EventResponse::Ok => {}
                EventResponse::Quit => break 'frame_loop,
                EventResponse::Restart => {
                    source = Source::new(cli.filename.as_deref(), cli.framerate).unwrap();
                },
                EventResponse::ChangeSource(path) => {
                    source = Source::new(Some(&path), cli.framerate).unwrap();
                }
                EventResponse::PlayPause => {
                    source.toggle_pause();
                },
            }
        }

        let img = source.next_frame();

        let frametime_avg = frame_times.iter().sum::<Duration>() / 300;

        let t0 = Instant::now();

        write!(terminal, "{}", termion::cursor::Goto(1, 1)).unwrap();
        write!(terminal, "{}", termion::clear::All).unwrap();
        // write!(stdout, "{}", renderer.render(&img, cli.char_height)).unwrap();
        write!(terminal, "{}", tui.render(img, cli.filename.as_deref().unwrap_or("None"), frametime_avg)).unwrap();
        write!(terminal, "{}", termion::cursor::Goto(tui.cursor_x(), tui.cursor_y())).unwrap();
        terminal.flush().unwrap();

        frame_times.pop_front();
        frame_times.push_back(Instant::now() - t0);
    }

    evt_thread.join().unwrap();
}

enum Message {
    Quit,
    NextMode,
    LastMode,
    Restart,
    PlayPause,
}

fn event_thread_old(tx: Sender<Message>) {
    for c in stdin().events() {
        let evt = c.unwrap();
        match evt {
            Event::Key(Key::Char('q')) => {
                tx.send(Message::Quit);
                break;
            },
            Event::Key(Key::Char('m')) => {
                tx.send(Message::NextMode);
            },
            Event::Key(Key::Char('M')) => {
                tx.send(Message::LastMode);
            },
            Event::Key(Key::Char('r')) => {
                tx.send(Message::Restart);
            },
            Event::Key(Key::Char('p')) => {
                tx.send(Message::PlayPause);
            },
            _ => {},
        }
    }
}

fn event_thread(term: Terminal<TermUtility>, tx: Sender<TermEvent>) {
    for (stop_after, event) in term.events() {
        tx.send(event).unwrap();
        if stop_after && *EVENT_THREAD_ACCEPT_EXIT.lock().unwrap() {
            break;
        }
    }
}
