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

#[derive(Parser)]
struct Cli {
    #[clap(validator = file_exists)]
    filename: String,
    #[clap(short, long, default_value_t = 30)]
    framerate: u32,
    #[clap(short = 'h', long, default_value_t = 2.2)]
    char_height: f32,
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

    let (tx, rx) = channel();
    let evt_thread = thread::Builder::new()
        .name("event".to_string())
        .spawn(move || event_thread(tx))
        .unwrap();

    let mut i = 0;
    'frame_loop: loop {
        for msg in rx.try_iter() {
            match msg {
                Message::Quit => break 'frame_loop,
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
        let (tw, th) = calc_dims(img.width(), img.height(), cli.char_height);
        img = image::imageops::resize(&img, tw as u32, th as u32, image::imageops::FilterType::Triangle);
        let mut frame = String::new();
        for j in 0..th {
            for k in 0..tw {
                let px = img.get_pixel(k, j).channels();
                //println!("{}, {}, {}", px.r, px.g, px.b);
                frame.push_str(&*"\u{2588}".truecolor(px[0], px[1], px[2]).to_string())
            }
            frame.push('\n');
        }
        frame.push_str("press 'q' to exit: ");

        write!(stdout, "{}", termion::cursor::Goto(1, 1));
        write!(stdout, "{}", termion::clear::All);
        write!(stdout, "{}", frame).unwrap();
        stdout.flush().unwrap();

        i += 1;
    }

    evt_thread.join();
}

fn calc_dims(imgw: u32, imgh: u32, char_height: f32) -> (u32, u32) {
    let (tw, th) = termion::terminal_size().unwrap();
    let term_ratio = (tw as f32 / (th as f32 - 1.0));
    let img_ratio = (imgw as f32 / imgh as f32) * char_height;
    if img_ratio > term_ratio {
        (tw as u32, std::cmp::min((tw as f32 / img_ratio) as u32, (th - 1) as u32))
    } else {
        (std::cmp::min(((th as f32 - 1.0) * img_ratio) as u32, tw as u32), th as u32)
    }
}

enum Message {
    Quit,
}

fn event_thread(tx: Sender<Message>) {
    for c in stdin().events() {
        let evt = c.unwrap();
        match evt {
            Event::Key(Key::Char('q')) => {
                tx.send(Message::Quit);
                break;
            },
            _ => {},
        }
    }
}