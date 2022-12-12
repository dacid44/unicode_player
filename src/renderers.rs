use std::iter;

use clap::ArgEnum;
use colored::Colorize;
use image::{Rgb, RgbImage};

use crate::tui::Area;

#[derive(Clone, ArgEnum)]
pub enum Renderer {
    PixelChar,
    HalfChar,
    Quarters,
    Braille,
}

impl Renderer {
    pub fn next_mode(&self) -> Self {
        match self {
            Self::PixelChar => Self::HalfChar,
            Self::HalfChar => Self::Quarters,
            Self::Quarters => Self::Braille,
            Self::Braille => Self::PixelChar
        }
    }

    pub fn last_mode(&self) -> Self {
        match self {
            Self::PixelChar => Self::Braille,
            Self::HalfChar => Self::PixelChar,
            Self::Quarters => Self::HalfChar,
            Self::Braille => Self::Quarters,
        }
    }

    fn subpixels(&self) -> (u32, u32) {
        match self {
            Self::PixelChar => (1, 1),
            Self::HalfChar => (1, 2),
            Self::Quarters => (2, 2),
            Self::Braille => (2, 4),
        }
    }

    pub(crate) fn name(&self) -> String {
        match self {
            Renderer::PixelChar => "Full Chars".to_string(),
            Renderer::HalfChar => "Half Chars".to_string(),
            Renderer::Quarters => "Quarters".to_string(),
            Renderer::Braille => "Braille".to_string(),
        }
    }

    fn calc_dims(&self, img_dims: (u32, u32), char_height: f32) -> (u32, u32) {
        let char_height = char_height * self.subpixels().0 as f32 / self.subpixels().1 as f32;
        let (tw, th) = {
            let mut dims = termion::terminal_size().unwrap();
            if dims.0 == 0 || dims.1 == 0 {
                dims = (80, 24)
            }
            (dims.0 as u32 * self.subpixels().0, dims.1 as u32 * self.subpixels().1)
        };
        let img_ratio = (img_dims.0 as f32 / img_dims.1 as f32) * char_height;
        if img_ratio > (tw as f32 / th as f32) {
            (
                tw as u32,
                std::cmp::min(
                    (tw as f32 / img_ratio / self.subpixels().1 as f32) as u32 * self.subpixels().1,
                    th as u32 * self.subpixels().1,
                )
            )
        } else {
            (
                std::cmp::min(
                    (th as f32 * img_ratio / self.subpixels().0 as f32) as u32 * self.subpixels().0,
                    tw as u32 * self.subpixels().0
                ),
                th as u32
            )
        }
    }

    fn calc_dims_fixed(&self, img_dims: (u32, u32), bounds: Area, char_height: f32) -> (Area, u32, u32) {
        let img_ratio = img_dims.0 as f32 / img_dims.1 as f32 * char_height;
        let bounds_ratio = bounds.aspect_ratio();

        fn fudge_even_odd(n: u32, cmp: u32) -> u32 {
            if n % 2 == cmp % 2 {
                n
            } else {
                n - 1
            }
        }

        if img_ratio > bounds_ratio {
            let h = fudge_even_odd((bounds.width as f32 / img_ratio).round() as u32, bounds.height).min(bounds.height);
            let gap = (bounds.height - h) / 2;
            (Area { width: bounds.width * self.subpixels().0, height: h * self.subpixels().1 }, 0, gap)
        } else {
            let w = fudge_even_odd((bounds.height as f32 * img_ratio).round() as u32, bounds.width).min(bounds.width);
            let gap = (bounds.width - w) / 2;
            (Area { width: w * self.subpixels().0, height: bounds.height * self.subpixels().1 }, gap, 0)
        }
    }

    pub(crate) fn render_player(&self, img: &RgbImage, bounds: Area, char_height: f32) -> Vec<String> {
        let (dims, gap_x, gap_y) = self.calc_dims_fixed((img.width(), img.height()), bounds, char_height);
        let scaled_img = image::imageops::resize(img, dims.width, dims.height, image::imageops::FilterType::Triangle);

        let vert_spacer = " ".repeat(bounds.width as usize);
        let horiz_spacer = " ".repeat(gap_x as usize);

        iter::repeat(vert_spacer.clone())
            .take(gap_y as usize)
            .chain(
                (0..dims.height)
                    .step_by(self.subpixels().1 as usize)
                    .map(|i| format!(
                        "{}{}{}",
                        horiz_spacer,
                        (0..dims.width)
                            .step_by(self.subpixels().0 as usize)
                            .map(|j| self.render_pixel(&scaled_img, (j, i))).collect::<String>(),
                        horiz_spacer,
                    ))
            )
            .chain(
                iter::repeat(vert_spacer)
                    .take(gap_y as usize)
            )
            .collect()
    }

    fn render_pixel(&self, img: &RgbImage, loc: (u32, u32)) -> String {
        match self {
            Self::PixelChar => {
                let px = img.get_pixel(loc.0, loc.1);
                "\u{2588}"
                    .truecolor(px[0], px[1], px[2])
                    .to_string()
            },
            Self::HalfChar => {
                let px = img.get_pixel(loc.0, loc.1);
                let px2 = img.get_pixel(loc.0, loc.1 + 1);
                "\u{2580}"
                    .truecolor(px[0], px[1], px[2])
                    .on_truecolor(px2[0], px2[1], px2[2])
                    .to_string()
            },
            Self::Quarters => {
                let px = img.get_pixel(loc.0, loc.1);
                let px2 = img.get_pixel(loc.0 + 1, loc.1);
                let px3 = img.get_pixel(loc.0, loc.1 + 1);
                let px4 = img.get_pixel(loc.0 + 1, loc.1 + 1);
                let extremes = get_extreme_colors(&[px, px2, px3, px4]);
                get_quarters_char((
                    is_closer_to_fg(px, extremes.0, extremes.1),
                    is_closer_to_fg(px2, extremes.0, extremes.1),
                    is_closer_to_fg(px3, extremes.0, extremes.1),
                    is_closer_to_fg(px4, extremes.0, extremes.1)
                ))
                    .truecolor(extremes.0[0], extremes.0[1], extremes.0[2])
                    .on_truecolor(extremes.1[0], extremes.1[1], extremes.1[2])
                    .to_string()
            },
            Self::Braille => {
                let pixels = [
                    img.get_pixel(loc.0, loc.1),
                    img.get_pixel(loc.0, loc.1 + 1),
                    img.get_pixel(loc.0, loc.1 + 2),
                    img.get_pixel(loc.0 + 1, loc.1),
                    img.get_pixel(loc.0 + 1, loc.1 + 1),
                    img.get_pixel(loc.0 + 1, loc.1 + 2),
                    img.get_pixel(loc.0, loc.1 + 3),
                    img.get_pixel(loc.0 + 1, loc.1 + 3),
                ];
                let extremes = get_extreme_colors(&pixels);
                let mut subpixels = [false; 8];
                for i in 0..8 {
                    subpixels[i] = is_closer_to_fg(pixels[i], extremes.0, extremes.1);
                }
                get_braille_char(subpixels)
                    .to_string()
                    .truecolor(extremes.0[0], extremes.0[1], extremes.0[2])
                    .on_truecolor(extremes.1[0], extremes.1[1], extremes.1[2])
                    .to_string()
            },
        }
    }
}

fn get_extreme_colors<'a>(colors: &[&'a Rgb<u8>]) -> (&'a Rgb<u8>, &'a Rgb<u8>) {
    let mut brightest = colors[0];
    let mut dimmest = colors[0];
    for color in colors.iter() {
        let brightness = calc_brightness(color);
        if brightness > calc_brightness(brightest) {
            brightest = color;
        }
        if brightness < calc_brightness(dimmest) {
            dimmest = color;
        }
    }
    (brightest, dimmest)
}

fn calc_brightness(color: &Rgb<u8>) -> f32 {
    0.2126 * (color[0] as f32) + 0.7152 * (color[1] as f32) + 0.0722 * (color[2] as f32)
}

fn is_closer_to_fg(color: &Rgb<u8>, fg: &Rgb<u8>, bg: &Rgb<u8>) -> bool {
    let brightness = calc_brightness(color);
    (brightness - calc_brightness(fg)).abs() <= (brightness - calc_brightness(bg)).abs()
}

fn get_quarters_char(subpixels: (bool, bool, bool, bool)) -> &'static str {
    match subpixels {  // tl, tr, bl, br
        (false, false, false, false) => " ",
        (false, false, false, true ) => "▗",
        (false, false, true , false) => "▖",
        (false, false, true , true ) => "▄",
        (false, true , false, false) => "▝",
        (false, true , false, true ) => "▐",
        (false, true , true , false) => "▞",
        (false, true , true , true ) => "▟",
        (true , false, false, false) => "▘",
        (true , false, false, true ) => "▚",
        (true , false, true , false) => "▌",
        (true , false, true , true ) => "▙",
        (true , true , false, false) => "▀",
        (true , true , false, true ) => "▜",
        (true , true , true , false) => "▛",
        (true , true , true , true ) => "█",
    }
}

fn get_braille_char(subpixels: [bool; 8]) -> char {
    let mut c: u32 = 10240;
    for (i, s) in subpixels.iter().enumerate() {
        if *s {
            c += 2u32.pow(i as u32)
        }
    }
    char::from_u32(c).unwrap()
}