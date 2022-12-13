use std::collections::VecDeque;
use std::io::prelude::*;
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use clap::lazy_static::lazy_static;
use clap::Parser;
use image::io::Reader as ImageReader;

use renderers::Renderer;

use crate::source::Source;
use crate::terminal::{TermEvent, TermUtility, Terminal};
use crate::tui::{EventResponse, Tui};

mod renderers;
mod source;
mod terminal;
mod tui;
mod youtube;

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
        Ok(())
    } else {
        Err("Invalid input file.".to_string())
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

    let renderer = cli.mode;
    let mut tui = Tui::new(renderer, cli.char_height, &terminal);

    let mut frame_times = VecDeque::from([Duration::new(0, 0); 300]);

    'frame_loop: loop {
        for event in rx.try_iter() {
            match tui.handle_event(event) {
                EventResponse::Ok => {}
                EventResponse::Quit => break 'frame_loop,
                EventResponse::Restart => {
                    source = Source::new(cli.filename.as_deref(), cli.framerate).unwrap();
                }
                EventResponse::ChangeSource(path) => {
                    source = Source::new(Some(&path), cli.framerate).unwrap();
                }
                EventResponse::PlayPause => {
                    source.toggle_pause();
                }
            }
        }

        let img = source.next_frame();

        let frametime_avg = frame_times.iter().sum::<Duration>() / 300;

        let t0 = Instant::now();

        terminal.move_cursor(1, 1).unwrap();
        terminal.clear().unwrap();
        write!(
            terminal,
            "{}",
            tui.render(
                img,
                cli.filename.as_deref().unwrap_or("None"),
                frametime_avg,
                &terminal
            )
        )
        .unwrap();
        terminal.move_cursor(tui.cursor_x(), tui.cursor_y()).unwrap();
        terminal.flush().unwrap();

        frame_times.pop_front();
        frame_times.push_back(Instant::now() - t0);
    }

    evt_thread.join().unwrap();
}

fn event_thread(term: Terminal<TermUtility>, tx: Sender<TermEvent>) {
    for (stop_after, event) in term.events() {
        tx.send(event).unwrap();
        if stop_after && *EVENT_THREAD_ACCEPT_EXIT.lock().unwrap() {
            break;
        }
    }
}
