use std::iter;
use std::collections::BTreeMap;
use sixel_tokenizer::SixelEvent;

use crate::{SixelColor, SixelImage, Pixel};

#[derive(Debug, Clone)]
pub struct SixelDeserializer {
    color_registers: BTreeMap<u8, SixelColor>,
    current_color: u8,
    sixel_cursor_y: usize,
    sixel_cursor_x: usize,
    pixels: Vec<Vec<Pixel>>,
}

impl SixelDeserializer {
    pub fn new() -> Self {
        SixelDeserializer {
            color_registers: BTreeMap::new(),
            current_color: 0, // this is totally undefined behaviour and seems like a free for all in general
            sixel_cursor_y: 0,
            sixel_cursor_x: 0,
            pixels: vec![vec![]], // start with one empty line
        }
    }
    pub fn create_image(&mut self) -> SixelImage {
        let pixels = std::mem::take(&mut self.pixels);
        let color_registers = std::mem::take(&mut self.color_registers);
        SixelImage {
            pixels,
            color_registers,
        }
    }
    pub fn handle_event(&mut self, event: SixelEvent) -> Result<(), &'static str> {
        match event {
            SixelEvent::ColorIntroducer { color_coordinate_system, color_number } => {
                match color_coordinate_system {
                    Some(color_coordinate_system) => {
                        // define a color in a register
                        let color = SixelColor::from(color_coordinate_system);
                        self.color_registers.insert(color_number, color);
                    },
                    None => {
                        // switch to register number
                        self.current_color = color_number;
                    }
                }
            }
            SixelEvent::RasterAttribute { pan: _, pad: _, ph, pv } => {
                // we ignore pan/pad because (reportedly) no-one uses them
                if let Some(pv) = pv {
                    self.pad_lines_vertically(pv);
                }
                if let Some(ph) = ph {
                    self.pad_lines_horizontally(ph);
                }
            }
            SixelEvent::Data { byte } => {
                self.make_sure_six_lines_exist_after_cursor();
                self.add_sixel_byte(byte, 1);
                self.sixel_cursor_x += 1;
            }
            SixelEvent::Repeat { repeat_count, byte_to_repeat } => {
                self.make_sure_six_lines_exist_after_cursor();
                self.add_sixel_byte(byte_to_repeat, repeat_count);
                self.sixel_cursor_x += repeat_count;
            }
            SixelEvent::Dcs { macro_parameter: _, inverse_background: _, horizontal_pixel_distance: _ } => {
                // noop
            }
            SixelEvent::GotoBeginningOfLine => {
                self.sixel_cursor_x = 0;
            }
            SixelEvent::GotoNextLine => {
                self.sixel_cursor_y += 6;
                self.sixel_cursor_x = 0;
            }
            SixelEvent::UnknownSequence(_) => {
                return Err("Corrupted Sixel sequence");
            }
            SixelEvent::End => {

            }
        }
        Ok(())
    }
    fn make_sure_six_lines_exist_after_cursor(&mut self) {
        let lines_to_add = (self.sixel_cursor_y + 6).saturating_sub(self.pixels.len());
        for _ in 0..lines_to_add {
            self.pixels.push(vec![]);
        }
    }
    fn add_sixel_byte(&mut self, byte: u8, repeat_count: usize) {
        let mut pixel_line_index_in_sixel = 0;
        for bit in SixelPixelIterator::new(byte.saturating_sub(63)) {
            let current_line = self.pixels.get_mut(self.sixel_cursor_y + pixel_line_index_in_sixel).unwrap();
            let new_pixel = Pixel {
                on: bit,
                color: self.current_color
            };
            for i in 0..repeat_count {
                match current_line.get_mut(self.sixel_cursor_x + i) {
                    Some(pixel_in_current_position) if bit => {
                        let _ = std::mem::replace(pixel_in_current_position, new_pixel);
                    },
                    None => {
                        current_line.push(new_pixel);
                    }
                    _ => {} // bit is off and pixel already exists, so noop
                }
            }
            pixel_line_index_in_sixel += 1;
        }
    }
    fn pad_lines_horizontally(&mut self, pad_until: usize) {
        let empty_pixel = Pixel {
            on: true,
            color: self.current_color
        };
        if self.pixels.len() < pad_until {
            let empty_line = vec![empty_pixel; pad_until];
            let lines_to_pad = pad_until - self.pixels.len();
            let line_padding = iter::repeat(empty_line).take(lines_to_pad);
            self.pixels.extend(line_padding);
        }
    }
    fn pad_lines_vertically(&mut self, pad_until: usize) {
        let empty_pixel = Pixel {
            on: true,
            color: self.current_color
        };
        for pixel_line in self.pixels.iter_mut() {
            if pixel_line.len() < pad_until {
                let pixel_count_to_pad = pad_until - pixel_line.len();
                let pixel_padding = iter::repeat(empty_pixel).take(pixel_count_to_pad);
                pixel_line.extend(pixel_padding);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SixelPixelIterator {
    sixel_byte: u8,
    current_mask: u8,
}
impl SixelPixelIterator {
    pub fn new(sixel_byte: u8) -> Self {
        SixelPixelIterator { sixel_byte, current_mask: 1 }
    }
}
impl Iterator for SixelPixelIterator {
    type Item = bool;
    fn next(&mut self) -> Option<Self::Item> {
        // iterate through the bits in a byte from right (least significant) to left (most
        // significant), eg. 89 => 1011001 => true, false, false, true, true, false, true
        let bit = self.sixel_byte & self.current_mask == self.current_mask;
        self.current_mask <<= 1;
        if self.current_mask == 128 {
            None
        } else {
            Some(bit)
        }
    }
}

