use colored::Colorize;
use hex_literal::hex;
use image::{Rgb, RgbImage};
use clap::ArgEnum;

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

    pub fn render(&self, img: &RgbImage, char_height: f32) -> String {
        let dims = self.calc_dims((img.width(), img.height()), char_height);
        let scaled_img = image::imageops::resize(img, dims.0, dims.1, image::imageops::FilterType::Triangle);
        let mut frame = String::new();
        for i in (0..dims.1).step_by(self.subpixels().1 as usize) {
            for j in (0..dims.0).step_by(self.subpixels().0 as usize) {
                frame.push_str(&*self.render_pixel(&scaled_img, (j, i)))
            }
            frame.push('\n');
        }
        frame.push_str("press 'm'/'M' to cycle mode, 'q' to exit: ");
        frame
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
            _ => "".to_string(),
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
    for i in 0..8 {
        if subpixels[i] {
            c += 2u32.pow(i as u32)
        }
    }
    char::from_u32(c).unwrap()
}