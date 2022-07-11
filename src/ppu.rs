use anyhow::Result;
use std::collections::VecDeque;

pub enum Mode {
    OamScan,
    Drawing,
    HBlank,
    VBlank
}

pub struct PixelData {
    color: u8,
    palette: u8,
    sprite_priority: u8,
    background_priority: u8
}

impl Default for Mode {
    fn default() -> Self {
        Self::OamScan
    }
}

pub struct Ppu {
    vram: [u8; 0x8192],
    lcd_control: u8,
    scy: u8,
    scx: u8,
    ly: u8,
    lyc: u8,
    wy: u8,
    wx: u8,
    bgp: u8,
    window_line_counter: u8,
    bg_fifo: VecDeque<PixelData>,
    frame_buffer: [[u8; 4]; 160 * 144],
    current_cycle: usize,
    mode: Mode
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            vram: [0; 0x8192],
            lcd_control: 0x80,
            scy: Default::default(),
            scx: Default::default(),
            ly: Default::default(),
            lyc: Default::default(),
            wy: Default::default(),
            wx: Default::default(),
            bgp: Default::default(),
            window_line_counter: Default::default(),
            bg_fifo: VecDeque::new(),
            frame_buffer: [[0; 4]; 160 * 144],
            current_cycle: Default::default(),
            mode: Default::default()
        }
    }

    pub fn tick(&mut self, cycle: u8) {
        let lcd7 = self.read_lcd_bit(7);
        if !lcd7 {
            return;
        }

        self.current_cycle += cycle as usize;
        if self.ly > 143 {
            self.mode = Mode::VBlank;
        }

        match self.mode {
            Mode::OamScan => {
                if self.current_cycle >= 80 {
                    self.mode = Mode::Drawing;
                }
                // 後で実装
                
            },
            Mode::Drawing => {
                if self.current_cycle >= 252 {
                    self.mode = Mode::HBlank;
                    self.fetch();
                }
            },
            Mode::HBlank => {
                if self.current_cycle >= 456 {
                    self.mode = Mode::OamScan;
                    self.current_cycle = 0;
                    self.ly += 1;
                }
            },
            Mode::VBlank => {
                if self.current_cycle >= 456 {
                    self.current_cycle = 0;
                    self.ly = (self.ly + 1) % 154;
                    if self.ly == 0 {
                        self.current_cycle = 0;
                        self.mode = Mode::OamScan;
                    }
                }
            }
        }
    }

    pub fn fetch(&mut self) {
        // あとでどこかに定数化する
        let black: [u8; 4] = [0x00, 0x00, 0x00, 0xff];
        let white: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
        let dark_gray: [u8; 4] = [0x1e, 0x51, 0x28, 0xff];
        let light_gray: [u8; 4] = [0xd8, 0xe9, 0xa8, 0xff];
        let color_palette: [[u8; 4]; 4] = [white, light_gray, dark_gray, black];

        let scan_line = self.ly;
        for x_position_counter in 0..160 {
            // bg fetch
            if self.bg_fifo.is_empty() {
                self.bg_fetch(scan_line, x_position_counter);
            }

            // sprite fetch

            // merge

            // push
            if x_position_counter == 0 {
                let discard = self.scx % 8;
                for _ in 0..discard {
                    self.bg_fifo.pop_front();
                }
            }


            let color_idx = self.bg_fifo.pop_front().unwrap().color;
            let color = color_palette[color_idx as usize];
            for i in 0..4 {
                self.frame_buffer[(scan_line as usize * 160 + x_position_counter as usize) as usize][i] = color[i];
            }
        }

    }
    

    pub fn render(&mut self, frame: &mut [u8]) -> Result<()> {
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let pixel_data = self.frame_buffer[i];
            pixel.copy_from_slice(&pixel_data);
        }
        Ok(())
    }

    pub fn bg_fetch(&mut self, scan_line: u8, x_coordinate: u8) {
        let lcd4 = self.read_lcd_bit(4);
        let lcd3 = self.read_lcd_bit(3);
        let lcd6 = self.read_lcd_bit(6);

        // fetch tile number
        let bg_tile_map_address: u16 = if lcd6 && !lcd3 { 0x1C00 } else { 0x1800 };

        let scx: u16 = self.scx as u16;
        let ly: u16 = self.ly as u16;
        let scy: u16 = self.scy as u16;
        // tile_map_idx = ((scx + x_coordinate) / 8) + ((ly + scy) / 8 * 32)
        let tile_map_idx = scx.wrapping_add(x_coordinate as u16).wrapping_div(8).wrapping_add(ly.wrapping_add(scy).wrapping_mul(4));
        let tile_number_address: u16 = tile_map_idx + bg_tile_map_address as u16;
        let tile_number = self.vram[tile_number_address as usize];

        // fetch tile data (low)
        let mut tile_address: usize = if lcd4 { tile_number as usize * 16 } else { (0x1000_i16 + (tile_number as i16)) as usize };
        let tile_vertical_offset = ((ly + scy) % 8) * 2;
        tile_address += tile_vertical_offset as usize;
        let lower_tile_data = self.vram[tile_address];

        // fetch tile data (high)
        let higher_tile_data = self.vram[tile_address + 1];

        // push fifo
        for bit in (0_u8..8).rev() {
            let top = if higher_tile_data & (1 << bit) == (1 << bit) {1_u8} else {0_u8};
            let bottom = if lower_tile_data & (1 << bit) == (1 << bit) {1_u8} else {0_u8};

            let pixel_color = top*2 + bottom;
            let pixel_data = PixelData {
                color: pixel_color,
                background_priority: 0,
                palette: 0,
                sprite_priority: 0
            };

            self.bg_fifo.push_back(pixel_data);
        }
    }

    pub fn write(&mut self, address: u16, data: u8) -> Result<()> {
        self.vram[address as usize] = data;
        Ok(())
    }

    pub fn read(&self, address: u16) -> Result<u8> {
        let data = self.vram[address as usize];
        Ok(data)
    }

    pub fn lcd_control_write(&mut self, data: u8) -> Result<()> {
        self.lcd_control = data;
        Ok(())
    }

    pub fn lcd_control_read(&self) -> Result<u8> {
        let data = self.lcd_control;
        Ok(data)
    }

    pub fn scy_write(&mut self, data: u8) -> Result<()> {
        self.scy = data;
        Ok(())
    }

    pub fn scy_read(&self) -> Result<u8> {
        let data = self.scy;
        Ok(data)
    }

    pub fn scx_write(&mut self, data: u8) -> Result<()> {
        self.scx = data;
        Ok(())
    }

    pub fn scx_read(&self) -> Result<u8> {
        let data = self.scx;
        Ok(data)
    }

    pub fn ly_write(&mut self, data: u8) -> Result<()> {
        self.ly = data;
        Ok(())
    }

    pub fn ly_read(&self) -> Result<u8> {
        let data = self.ly;
        Ok(data)
    }

    pub fn lyc_write(&mut self, data: u8) -> Result<()> {
        self.lyc = data;
        Ok(())
    }

    pub fn lyc_read(&self) -> Result<u8> {
        let data = self.lyc;
        Ok(data)
    }

    pub fn wy_write(&mut self, data: u8) -> Result<()> {
        self.wy = data;
        Ok(())
    }

    pub fn wy_read(&self) -> Result<u8> {
        let data = self.wy;
        Ok(data)
    }

    pub fn wx_write(&mut self, data: u8) -> Result<()> {
        self.wx = data;
        Ok(())
    }

    pub fn wx_read(&self) -> Result<u8> {
        let data = self.wx;
        Ok(data)
    }

    pub fn bgp_read(&self) -> Result<u8> {
        let data = self.bgp;
        Ok(data)
    }

    pub fn bgp_write(&mut self, data: u8) -> Result<()> {
        self.bgp = data;
        Ok(())
    }

    fn read_lcd_bit(&self, bit: u8) -> bool {
        return &self.lcd_control & (1 << bit) == (1 << bit);
    }
}
