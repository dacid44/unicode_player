use std::collections::VecDeque;
use std::fs::File;
use std::io::prelude::*;
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use clap::Parser;
use image::io::Reader as ImageReader;
use lazy_static::lazy_static;

use renderers::Renderer;

use crate::source::Source;
use crate::terminal::{TermEvent, TermUtility, Terminal};
use crate::tui::{Area, EventResponse, Tui};

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
    #[arg(short, long, default_value_t = 30)]
    framerate: u32,
    #[arg(short = 'h', long, default_value_t = 2.2)]
    char_height: f32,
    #[arg(short, long, value_enum, default_value_t = Renderer::PixelChar)]
    mode: Renderer,
    #[arg(short, long, requires = "size", requires = "filename")]
    output: Option<String>,
    #[arg(short, long, requires = "output", value_parser = parse_dims)]
    size: Option<(u32, u32)>,
}

fn file_exists(filename: &str) -> Result<(), String> {
    if std::path::Path::new(filename).is_file() {
        Ok(())
    } else {
        Err("Invalid input file.".to_string())
    }
}

fn parse_dims(dims: &str) -> Result<(u32, u32), String> {
    let parts = dims.split(&['x', 'X', ':', ',']).collect::<Vec<&str>>();
    if parts.len() != 2 {
        return Err("Failed to parse dims".to_string());
    }
    if let (Ok(w), Ok(h)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
        Ok((w, h))
    } else {
        Err("Failed to parse dims".to_string())
    }
}

fn main() {
    let cli = Cli::parse();

    let mut source = Source::new(
        cli.filename.as_deref(),
        cli.framerate,
        cli.output.is_some(),
    ).unwrap();

    let renderer = cli.mode;

    if let Some(output) = cli.output {
        let mut file = File::create(output).unwrap();

        let size = cli.size.unwrap();
        let area = Area { width: size.0, height: size.1 };

        write!(
            file,
            "{{\"framerate\": {}, \"width\": {}, \"height\": {}}}",
            cli.framerate,
            size.0,
            size.1,
        ).unwrap();

        while !source.finished {
            let frame = renderer.render_player(source.next_frame(), area, cli.char_height);
            write!(file, "\n{}", frame.join("")).unwrap();
        }

        file.flush().unwrap();
    } else {
        let mut terminal = Terminal::new_crossterm();

        let (tx, rx) = channel();
        let evt_thread = thread::Builder::new()
            .name("event".to_string())
            .spawn({
                let terminal = terminal.utility();
                move || event_thread(terminal, tx)
            })
            .unwrap();

        let mut tui = Tui::new(renderer, cli.char_height, &terminal);

        let mut frame_times = VecDeque::from([Duration::new(0, 0); 300]);

        'frame_loop: loop {
            for event in rx.try_iter() {
                match tui.handle_event(event) {
                    EventResponse::Ok => {}
                    EventResponse::Quit => break 'frame_loop,
                    EventResponse::Restart => {
                        source = Source::new(cli.filename.as_deref(), cli.framerate, false).unwrap();
                    }
                    EventResponse::ChangeSource(path) => {
                        source = Source::new(Some(&path), cli.framerate, false).unwrap();
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
}

fn event_thread(term: Terminal<TermUtility>, tx: Sender<TermEvent>) {
    for (stop_after, event) in term.events() {
        tx.send(event).unwrap();
        if stop_after && *EVENT_THREAD_ACCEPT_EXIT.lock().unwrap() {
            break;
        }
    }
}
