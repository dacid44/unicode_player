use std::borrow::BorrowMut;
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::iter;
use colored::Colorize;
use image::RgbImage;
use termion::event::{Event, Key};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
use crate::{EVENT_THREAD_ACCEPT_EXIT, Renderer, youtube};

const HELP_TEXT: &'static str = "Press 'm'/'M' to cycle mode, 'q' to exit, 'r' to restart, 'p' to play/pause: ";

pub(crate) struct Tui {
    player: Player,
    search: Search,
    focus: TuiFocus,
    bounds: Area,
    cursor_pos: (u16, u16),
    char_height: f32,
}

impl Tui {
    pub(crate) fn new(renderer: Renderer, char_height: f32) -> Self {
        let mut tui = Self {
            player: Player::new(0, 0, renderer),
            search: Search::new(0, 0),
            focus: TuiFocus::Player,
            bounds: Area { width: 0, height: 0 },
            cursor_pos: (0, 0),
            char_height
        };
        tui.update_size();
        tui
    }

    pub(crate) fn update_size(&mut self) {
        let mut dims = termion::terminal_size().unwrap();
        if dims.0 == 0 || dims.1 == 0 {
            dims = (80, 24)
        }

        self.bounds.width = dims.0 as u32;
        self.bounds.height = dims.1 as u32;
        self.player.update_size(self.bounds.width - 40, self.bounds.height - 5);
        self.search.update_size(40, self.bounds.height);
    }

    pub(crate) fn cursor_x(&self) -> u16 {
        self.cursor_pos.0
    }

    pub(crate) fn cursor_y(&self) -> u16 {
        self.cursor_pos.1
    }

    pub(crate) fn handle_event(&mut self, event: Event) -> EventResponse {
        if matches!(event, Event::Key(Key::Char('\t'))) {
            self.focus = self.focus.next_focus();
            *EVENT_THREAD_ACCEPT_EXIT.lock().unwrap() = self.focus.should_exit();
            return EventResponse::Ok;
        }

        match self.focus {
            TuiFocus::Player => match event {
                Event::Key(Key::Char('q')) => return EventResponse::Quit,
                Event::Key(Key::Char('m')) => self.player.next_renderer(),
                Event::Key(Key::Char('M')) => self.player.last_renderer(),
                Event::Key(Key::Char('r')) => return EventResponse::Restart,
                Event::Key(Key::Char('p')) => return EventResponse::PlayPause,
                _ => {},
            }
            TuiFocus::Search => match event {
                Event::Key(Key::Backspace) => self.search.handle_backspace(),
                Event::Key(Key::Down) => self.search.handle_arrow_down(),
                Event::Key(Key::Up) => self.search.handle_arrow_up(),
                Event::Key(Key::Char('\n')) => {
                    if let Some(path) = self.search.handle_enter() {
                        return EventResponse::ChangeSource(path);
                    }
                },
                Event::Key(Key::Char(c)) => self.search.handle_char(c),
                _ => {},
            }
        }

        EventResponse::Ok
    }

    pub(crate) fn render(&mut self, img: &RgbImage, path: &str) -> String {
        let mut frame = self.player.render(img, self.char_height);


        let renderer_name = self.player.renderer.name();

        let longest = (path.width_cjk() + 15)
            .max(renderer_name.len() + 20)
            .max(HELP_TEXT.len() + 3);

        let info_spacer = " ".repeat(self.player.bounds.width as usize - (longest + 2));

        frame.extend(
            [
                format!("╔{}╗{}", "═".repeat(longest), info_spacer.clone()),
                format!("║ Now Playing: {}{}║{}", path, " ".repeat(longest - path.width_cjk() - 14), info_spacer.clone()),
                format!("║ Current Renderer: {}{}║{}", renderer_name, " ".repeat(longest - renderer_name.len() - 19), info_spacer.clone()),
                format!("║ {}{}║{}", HELP_TEXT, " ".repeat(longest - HELP_TEXT.len() - 1), info_spacer.clone()),
                format!("╚{}╝{}", "═".repeat(longest), info_spacer),
            ].into_iter()
        );

        for (line, search_line) in frame.iter_mut().zip(self.search.draw().iter()) {
            line.push_str(search_line);
        }

        self.cursor_pos = match self.focus {
            TuiFocus::Player => (
                (HELP_TEXT.len() + 3) as u16,
                (frame.len() - 1) as u16,
            ),
            TuiFocus::Search => (
                self.player.bounds.width as u16 + self.search.cursor_x(),
                self.search.cursor_y(),
            ),
        };

        frame.join("\r\n")
    }
}

#[derive(Copy, Clone)]
enum TuiFocus {
    Player,
    Search,
}

impl TuiFocus {
    fn next_focus(&self) -> Self {
        match self {
            Self::Player => Self::Search,
            Self::Search => Self::Player,
        }
    }

    fn should_exit(&self) -> bool {
        match self {
            TuiFocus::Player => true,
            TuiFocus::Search => false,
        }
    }
}

#[derive(Clone)]
pub(crate) enum EventResponse {
    Ok,
    Quit,
    Restart,
    ChangeSource(String),
    PlayPause,
}

#[derive(Copy, Clone)]
pub(crate) struct Area {
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl Area {
    pub(crate) fn aspect_ratio(&self) -> f32 {
        self.width as f32 / self.height as f32
    }

    pub(crate) fn without_border(&self) -> Self {
        Self { width: self.width - 2, height: self.height - 2 }
    }
}

struct Player {
    bounds: Area,
    renderer: Renderer,
}

impl Player {
    fn new(width: u32, height: u32, renderer: Renderer) -> Self {
        Self { bounds: Area { width, height }, renderer }
    }

    fn update_size(&mut self, width: u32, height: u32) {
        self.bounds.width = width;
        self.bounds.height = height;
    }

    fn next_renderer(&mut self) {
        self.renderer = self.renderer.next_mode();
    }

    fn last_renderer(&mut self) {
        self.renderer = self.renderer.last_mode();
    }

    fn render(&self, img: &RgbImage, char_height: f32) -> Vec<String> {
        let inner_bounds = self.bounds.without_border();
        let frame = self.renderer.render_player(img, inner_bounds, char_height);

        iter::once(format!("╭{}╮", "─".repeat(inner_bounds.width as usize)))
            .chain(
                frame.into_iter().map(|line| format!("│{}│", line))
            )
            .chain(iter::once(format!("╰{}╯", "─".repeat(inner_bounds.width as usize))))
            .collect()
    }
}

struct Search {
    bounds: Area,
    selected: Option<usize>,
    query: String,
    results: Vec<SearchResult>,
}

impl Search {
    fn new(width: u32, height: u32) -> Self {
        Self { bounds: Area { width, height }, selected: None, query: "".to_string(), results: Vec::new() }
    }

    fn update_size(&mut self, width: u32, height: u32) {
        self.bounds.width = width;
        self.bounds.height = height;
    }

    fn cursor_x(&self) -> u16 {
        if self.selected.is_some() {
            2
        } else {
            let query_box_width = self.bounds.width as usize - 13;
            12 + self.query.len().min(query_box_width) as u16
        }
    }

    fn cursor_y(&self) -> u16 {
        if let Some(i) = self.selected {
            i as u16 * 3 + 4
        } else {
            2
        }
    }

    fn handle_char(&mut self, c: char) {
        self.query.push(c);
    }

    fn handle_backspace(&mut self) {
        self.query.pop();
    }

    fn handle_arrow_down(&mut self) {
        if let Some(i) = self.selected.as_mut() {
            let last_result = ((self.bounds.height as usize - 6) / 3).min(self.results.len() - 1);

            if *i < last_result {
                *i += 1;
            }
        } else if !self.results.is_empty() {
            self.selected = Some(0)
        }
    }

    fn handle_arrow_up(&mut self) {
        if let Some(i) = self.selected.as_mut() {
            if *i == 0 {
                self.selected = None;
            } else {
                *i -= 1;
            }
        }
    }

    fn handle_enter(&mut self) -> Option<String> {
        if let Some(i) = self.selected {
            return Some(self.results.get(i)?.path.clone());
        }

        match youtube::search(&self.query) {
            Ok(results) => {
                self.results = results;
            }
            Err(err) => {
                self.results = vec![SearchResult { title: err.to_string(), uploader: String::new(), path: String::new() } ];
            }
        }

        None
    }

    fn draw(&self) -> Vec<String> {
        let query_box_width = self.bounds.width as isize - 13;

        let truncated_query = self.query.chars()
            .skip((self.query.len() as isize - query_box_width).max(0) as usize)
            .take(query_box_width as usize)
            .collect::<String>();

        let num_results = (self.bounds.height as usize - 6) / 3 + 1;

        let mut results = self.results.iter();

        let rendered_results: Vec<String> = if self.results.is_empty() {
            None
        } else {
            // We just checked that the results vec is not empty
            Some(results.next().unwrap().draw(self.bounds.width, true).into_iter())
        }
            .into_iter()
            .chain(
                results.take(num_results - 1)
                    .map(|r| r.draw(self.bounds.width, false).into_iter())
            )
            .flatten()
            .collect();

        let rendered_results_len = rendered_results.len();

        iter::once(format!("╔════════╦{}╗", "═".repeat(self.bounds.width as usize - 11)))
            .chain(iter::once(
                format!(
                    "║ Search ║ {}{} ║",
                    truncated_query,
                    " ".repeat(query_box_width as usize - truncated_query.len()),
                )
            ))
            .chain(iter::once(
                format!("╠════════╩{}╣", "═".repeat(self.bounds.width as usize - 11))
            ))
            .chain(rendered_results.into_iter())
            .chain(iter::repeat(format!("║{}║", " ".repeat(self.bounds.width as usize - 2)))
                .take(self.bounds.height as usize - 4 - rendered_results_len)
            )
            .chain(iter::once(
                format!("╚{}╝", "═".repeat(self.bounds.width as usize - 2))
            ))
            .collect()
    }
}

#[derive(Debug)]
pub(crate) struct SearchResult {
    pub(crate) title: String,
    pub(crate) uploader: String,
    pub(crate) path: String,
    // pub(crate) description: String,
}

impl SearchResult {
    fn draw(&self, width: u32, is_first: bool) -> Vec<String> {
        let mut frame: Vec<String> = if is_first {
            Vec::new()
        } else {
            vec![format!("╟{}╢", "─".repeat(width as usize - 2))]
        };

        let display_area = width as usize - 4;

        let title_string = self.title.graphemes(true)
            .scan(0_usize, |width, grapheme| {
                *width += grapheme.width_cjk();
                if *width > display_area { None } else { Some(grapheme) }
            })
            .collect::<String>();
        let uploader_string = self.uploader.graphemes(true)
            .scan(0_usize, |width, grapheme| {
                *width += grapheme.width_cjk();
                if *width > display_area { None } else { Some(grapheme) }
            })
            .collect::<String>();

        let mut f = OpenOptions::new()
            .write(true)
            .append(true)
            .open("5.txt")
            .unwrap();
        writeln!(f, "{:?}", self).unwrap();

        frame.push(format!("║ {}{} ║", title_string.bold(), " ".repeat(display_area - title_string.width_cjk())));

        frame.push(format!("║ {}{} ║", uploader_string, " ".repeat(display_area - uploader_string.width_cjk())));

        frame
    }
}