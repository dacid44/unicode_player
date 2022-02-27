mod renderers;

use std::io::prelude::*;
use std::process::{Command, Stdio};
use iter_read::IterRead;
use colored::{Colorize, ColoredString};
use image::io::Reader as ImageReader;
use std::io::{Cursor, stdin, stdout};
use image::Pixel;
use termion::event::{Event, Key};
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use std::thread;
use std::sync::mpsc::{channel, Sender};
use clap::Parser;

use renderers::Renderer;

#[derive(Parser)]
struct Cli {
    #[clap(validator = file_exists)]
    filename: String,
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

    let process = match Command::new("ffmpeg")
        .args(&[
            "-re", "-i", &cli.filename,
            "-f", "image2pipe", "-c:v", "bmp", "-vf", &format!("fps={}", cli.framerate), "-",
            "-f", "pulse", "\"unicode_player\""
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn() {
        Err(why) => panic!("couldn't spawn ffmpeg: {}", why),
        Ok(process) => process,
    };
    let mut pipe = process.stdout.unwrap();

    let mut stdout = AlternateScreen::from(stdout()).into_raw_mode().unwrap();
    // let mut stdout = stdout().into_raw_mode().unwrap();

    let (tx, rx) = channel();
    let evt_thread = thread::Builder::new()
        .name("event".to_string())
        .spawn(move || event_thread(tx))
        .unwrap();

    let mut renderer = cli.mode;

    let mut i = 0;
    'frame_loop: loop {
        for msg in rx.try_iter() {
            match msg {
                Message::Quit => break 'frame_loop,
                Message::NextMode => renderer = renderer.next_mode(),
                Message::LastMode => renderer = renderer.last_mode(),
            }
        }

        let mut start: [u8; 6] = [0; 6];
        match pipe.read_exact(&mut start) {
            Ok(_) => {}
            Err(_) => { break; }
        }
        let bmp_length = u32::from_le_bytes(start[2..6].try_into().unwrap());
        let mut remaining_bytes: Vec<u8> = std::iter::repeat(0).take((bmp_length - 6) as usize).collect();
        match pipe.read_exact(&mut remaining_bytes[0..(bmp_length - 6) as usize]) {
            Ok(_) => {}
            Err(_) => { break; }
        }
        let image_bytes: Vec<u8> = start.into_iter().chain(remaining_bytes.into_iter()).collect();
        let mut img = ImageReader::with_format(Cursor::new(image_bytes), image::ImageFormat::Bmp)
            .decode().unwrap().to_rgb8();

        // let (tw, th) = termion::terminal_size().unwrap();
        // let (tw, th) = (tw as u32, th as u32 - 1);

        write!(stdout, "{}", termion::cursor::Goto(1, 1));
        write!(stdout, "{}", termion::clear::All);
        write!(stdout, "{}", renderer.render(&img, cli.char_height)).unwrap();
        stdout.flush().unwrap();

        i += 1;
    }

    evt_thread.join();
}

enum Message {
    Quit,
    NextMode,
    LastMode,
}

fn event_thread(tx: Sender<Message>) {
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
            _ => {},
        }
    }
}