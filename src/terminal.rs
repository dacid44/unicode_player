use std::borrow::BorrowMut;
use std::io::{Read, Stdin, stdin, Stdout, stdout, Write};
use crossterm::event::{Event as CtEvent, KeyEvent as CtKeyEvent, KeyEventKind as CtKeyEventKind, KeyCode as CtKeyCode};
use termion::input::TermRead;
use termion::event::{Event, Key, Event as TmEvent, Key as TmKey};


pub(crate) trait TermWrite { }

// TODO: Maybe change to two separate types and compile to one or the other based on OS
pub(crate) enum TermWriter {
    Termion(termion::raw::RawTerminal<termion::screen::AlternateScreen<Stdout>>),
    Crossterm(Stdout),
}

impl TermWrite for TermWriter { }

impl Drop for TermWriter {
    fn drop(&mut self) {
        match self {
            Self::Termion(_) => { },
            Self::Crossterm(w) => {
                crossterm::terminal::disable_raw_mode().unwrap();
                crossterm::execute!(w, crossterm::terminal::LeaveAlternateScreen).unwrap();
            }
        }
    }
}

pub(crate) struct TermUtility;

impl TermWrite for TermUtility { }

#[derive(Copy, Clone, Debug)]
pub(crate) enum TermKind {
    Termion,
    Crossterm,
}

pub(crate) enum TermEventStream {
    Termion(termion::input::Events<Stdin>),
    Crossterm
}

impl Iterator for TermEventStream {
    type Item = (bool, TermEvent);

    fn next(&mut self) -> Option<Self::Item> {
        Some(loop {
            if let Some(term_event) = match self {
                Self::Termion(events) => TermEvent::from_termion(events.next()?.unwrap()),
                Self::Crossterm => TermEvent::from_crossterm(crossterm::event::read().unwrap()),
            } {
                break (term_event.detect_quit(), term_event)
            }
        })
    }
}

pub(crate) struct Terminal<T: TermWrite> {
    pub(crate) kind: TermKind,
    writer: T,
}

impl<T: TermWrite> Terminal<T> {
    pub(crate) fn utility(&self) -> Terminal<TermUtility> {
        Terminal {
            writer: TermUtility,
            kind: self.kind,
        }
    }

    pub(crate) fn size(&self) -> (u16, u16) {
        match self.kind {
            TermKind::Termion => termion::terminal_size().unwrap(),
            TermKind::Crossterm => crossterm::terminal::size().unwrap(),
        }
    }

    pub(crate) fn events(&self) -> TermEventStream {
        match self.kind {
            TermKind::Termion => TermEventStream::Termion(stdin().events()),
            TermKind::Crossterm => TermEventStream::Crossterm,
        }
    }
}

impl Terminal<TermWriter> {
    pub(crate) fn new_termion() -> Self {
        Self {
            writer: TermWriter::Termion(
                termion::raw::IntoRawMode::into_raw_mode(
                    termion::screen::AlternateScreen::from(stdout())
                )
                    .unwrap()
            ),
            kind: TermKind::Termion,
        }
    }

    pub(crate) fn new_crossterm() -> Self {
        let mut writer = stdout();
        crossterm::execute!(writer, crossterm::terminal::EnterAlternateScreen).unwrap();
        crossterm::terminal::enable_raw_mode().unwrap();
        Self {
            writer: TermWriter::Crossterm(writer),
            kind: TermKind::Termion,
        }
    }
}

impl Write for Terminal<TermWriter> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self.writer.borrow_mut() {
            TermWriter::Termion(w) => w.write(buf),
            TermWriter::Crossterm(w) => w.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self.writer.borrow_mut() {
            TermWriter::Termion(w) => w.flush(),
            TermWriter::Crossterm(w) => w.flush(),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum TermEvent {
    Char(char),
    Backspace,
    Up,
    Down,
    Left,
    Right,
}

impl TermEvent {
    fn from_termion(event: TmEvent) -> Option<Self> {
        match event {
            TmEvent::Key(TmKey::Char(c)) => Some(Self::Char(c)),
            TmEvent::Key(TmKey::Backspace) => Some(Self::Backspace),
            TmEvent::Key(TmKey::Up) => Some(Self::Up),
            TmEvent::Key(TmKey::Down) => Some(Self::Down),
            TmEvent::Key(TmKey::Left) => Some(Self::Left),
            TmEvent::Key(TmKey::Right) => Some(Self::Right),
            _ => None,
        }
    }

    fn from_crossterm(event: CtEvent) -> Option<Self> {
        if let CtEvent::Key(CtKeyEvent { code: key_code, kind: CtKeyEventKind::Release, .. }) = event {
            match key_code {
                CtKeyCode::Char(c) => Some(Self::Char(c)),
                CtKeyCode::Backspace => Some(Self::Backspace),
                CtKeyCode::Up => Some(Self::Up),
                CtKeyCode::Down => Some(Self::Down),
                CtKeyCode::Left => Some(Self::Left),
                CtKeyCode::Right => Some(Self::Right),
                _ => None,
            }
        } else {
            None
        }
    }

    fn detect_quit(&self) -> bool {
        matches!(self, Self::Char('q'))
    }
}

impl TryFrom<TmEvent> for TermEvent {
    type Error = ();

    fn try_from(value: TmEvent) -> Result<Self, Self::Error> {
        Self::from_termion(value).ok_or(())
    }
}

impl TryFrom<CtEvent> for TermEvent {
    type Error = ();

    fn try_from(value: CtEvent) -> Result<Self, Self::Error> {
        Self::from_crossterm(value).ok_or(())
    }
}