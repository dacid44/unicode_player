mod renderers;
mod source;
mod tui;
mod youtube;

use std::io::prelude::*;
use std::process::{Command, Stdio};
use iter_read::IterRead;
use colored::{Colorize, ColoredString};
use image::io::Reader as ImageReader;
use std::io::{Cursor, stdin, stdout};
use image::{Pixel, RgbImage};
use termion::event::{Event, Key};
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use std::thread;
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;
use std::time::Duration;
use clap::lazy_static::lazy_static;
use clap::Parser;

use renderers::Renderer;
use crate::source::Source;
use crate::tui::{EventResponse, Tui};

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

    // let process = match Command::new("ffmpeg")
    //     .args(&[
    //         "-re", "-i", &cli.filename,
    //         "-f", "image2pipe", "-c:v", "bmp", "-vf", &format!("fps={}", cli.framerate), "-",
    //         "-f", "pulse", "\"unicode_player\""
    //     ])
    //     .stdout(Stdio::piped())
    //     .stderr(Stdio::null())
    //     .spawn() {
    //     Err(why) => panic!("couldn't spawn ffmpeg: {}", why),
    //     Ok(process) => process,
    // };
    // let mut pipe = process.stdout.unwrap();
    let mut source = Source::new(cli.filename.as_deref(), cli.framerate).unwrap();

    let mut stdout = AlternateScreen::from(stdout()).into_raw_mode().unwrap();
    // let mut stdout = stdout().into_raw_mode().unwrap();

    let (tx, rx) = channel();
    let evt_thread = thread::Builder::new()
        .name("event".to_string())
        .spawn(move || event_thread(tx))
        .unwrap();

    let mut renderer = cli.mode;
    let mut tui = Tui::new(renderer.clone(), cli.char_height);

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

        write!(stdout, "{}", termion::cursor::Goto(1, 1)).unwrap();
        write!(stdout, "{}", termion::clear::All).unwrap();
        // write!(stdout, "{}", renderer.render(&img, cli.char_height)).unwrap();
        write!(stdout, "{}", tui.render(img, cli.filename.as_deref().unwrap_or("None"))).unwrap();
        write!(stdout, "{}", termion::cursor::Goto(tui.cursor_x(), tui.cursor_y())).unwrap();
        stdout.flush().unwrap();
    }

    evt_thread.join();
}

enum Message {
    Quit,
    NextMode,
    LastMode,
    Restart,
    PlayPause,
}

// fn event_thread(tx: Sender<Message>) {
//     for c in stdin().events() {
//         let evt = c.unwrap();
//         match evt {
//             Event::Key(Key::Char('q')) => {
//                 tx.send(Message::Quit);
//                 break;
//             },
//             Event::Key(Key::Char('m')) => {
//                 tx.send(Message::NextMode);
//             },
//             Event::Key(Key::Char('M')) => {
//                 tx.send(Message::LastMode);
//             },
//             Event::Key(Key::Char('r')) => {
//                 tx.send(Message::Restart);
//             },
//             Event::Key(Key::Char('p')) => {
//                 tx.send(Message::PlayPause);
//             },
//             _ => {},
//         }
//     }
// }

fn event_thread(tx: Sender<Event>) {
    for c in stdin().events() {
        let event = c.unwrap();
        let stop_after = matches!(event, Event::Key(Key::Char('q')));
        tx.send(event).unwrap();
        if stop_after && *EVENT_THREAD_ACCEPT_EXIT.lock().unwrap() {
            break;
        }
    }
}