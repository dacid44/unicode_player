use std::error::Error;
use std::{io, thread};
use std::io::{Cursor, Read, Stderr};
use std::process::{Command, Stdio};
use std::time::Duration;
use image::{Rgb, RgbImage};
use crate::ImageReader;

pub(crate) struct Source {
    source_stream: SourceStream,
    paused: bool,
    finished: bool,
    framerate: u32,
    last_frame: RgbImage,
}

impl Source {
    pub(crate) fn new(path: Option<&str>, framerate: u32) -> Result<Self, Box<dyn Error>> {
        if let Some(path) = path {
            if path.contains("http") {
                let mut ytdl_process = Command::new("yt-dlp")
                    .args(&["-o", "-", path])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()?;

                let mut ffmpeg_process = Command::new("ffmpeg")
                    .args(&[
                        "-re", "-i", "-",
                        "-f", "image2pipe", "-c:v", "bmp", "-vf", &format!("fps={}", framerate), "-",
                        "-f", "pulse", "\"unicode_player\""
                    ])
                    .stdin(Stdio::from(ytdl_process.stdout.take().ok_or("Couldn't get yt-dlp stdout")?))
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()?;

                let stream = ffmpeg_process.stdout.take().ok_or("Couldn't get ffmpeg stdout")?;

                Ok(Self {
                    source_stream: SourceStream::YouTube { ytdl: ytdl_process, ffmpeg: ffmpeg_process, stream },
                    paused: false,
                    finished: false,
                    framerate,
                    last_frame: blank_frame(),
                })
            } else {
                let mut ffmpeg_process = Command::new("ffmpeg")
                    .args(&[
                        "-re", "-i", path,
                        "-f", "image2pipe", "-c:v", "bmp", "-vf", &format!("fps={}", framerate), "-",
                        "-f", "pulse", "\"unicode_player\""
                    ])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()?;

                let stream = ffmpeg_process.stdout.take().ok_or("Couldn't get ffmpeg stdout")?;

                Ok(Self {
                    source_stream: SourceStream::File { ffmpeg: ffmpeg_process, stream },
                    paused: false,
                    finished: false,
                    framerate,
                    last_frame: blank_frame(),
                })
            }
        } else {
            Ok(Self {
                source_stream: SourceStream::Blank,
                paused: false,
                finished: false,
                framerate,
                last_frame: blank_frame()
            })
        }
    }

    pub(crate) fn toggle_pause(&mut self) {
        self.paused = !self.paused
    }

    pub(crate) fn next_frame(&mut self) -> &RgbImage {
        if self.paused || self.finished {
            thread::sleep(Duration::from_secs_f32(1.0 / self.framerate as f32));
            return &self.last_frame;
        }

        if let Some(frame) = self.source_stream.next_frame(self.framerate) {
            self.last_frame = frame;
        } else {
            self.finished = true;
        }

        &self.last_frame
    }
}

enum SourceStream {
    Blank,
    File {
        ffmpeg: std::process::Child,
        stream: std::process::ChildStdout,
    },
    YouTube {
        ytdl: std::process::Child,
        ffmpeg: std::process::Child,
        stream: std::process::ChildStdout,
    },
}

impl SourceStream {
    fn stop(&mut self) {
        match self {
            SourceStream::Blank => {}
            SourceStream::File { ffmpeg, .. } => {
                ffmpeg.kill();
            }
            SourceStream::YouTube { ytdl, ffmpeg, .. } => {
                ytdl.kill();
                ffmpeg.kill();
            }
        }
    }

    fn next_frame(&mut self, framerate: u32) -> Option<RgbImage> {
        let stream = match self {
            SourceStream::Blank => {
                thread::sleep(Duration::from_secs_f32(1.0 / framerate as f32));
                return Some(blank_frame());
            },
            SourceStream::File { stream, .. } => stream,
            SourceStream::YouTube { stream, .. } => stream,
        };

        let mut start: [u8; 6] = [0; 6];
        match stream.read_exact(&mut start) {
            Ok(_) => {}
            // Err(_) => { break; }
            Err(_) => {
                return None;
            }
        }
        let bmp_length = u32::from_le_bytes(start[2..6].try_into().unwrap());
        let mut remaining_bytes: Vec<u8> = std::iter::repeat(0).take((bmp_length - 6) as usize).collect();
        match stream.read_exact(&mut remaining_bytes[0..(bmp_length - 6) as usize]) {
            Ok(_) => {}
            // Err(_) => { break; }
            Err(_) => {
                return None;
            }
        }
        let image_bytes: Vec<u8> = start.into_iter().chain(remaining_bytes.into_iter()).collect();
        Some(ImageReader::with_format(Cursor::new(image_bytes), image::ImageFormat::Bmp)
            .decode().unwrap().to_rgb8())
    }
}

impl Drop for SourceStream {
    fn drop(&mut self) {
        self.stop();
    }
}

fn blank_frame() -> RgbImage {
    RgbImage::from_pixel(1, 1, Rgb([0, 0, 0]))
}