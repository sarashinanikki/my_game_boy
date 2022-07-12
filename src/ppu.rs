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
                self.window_line_counter = 0;
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
            // check is window rendered
            if self.is_window_rendering(x_position_counter) {
                self.bg_fifo.clear();
                self.window_fetch(x_position_counter);
            }

            // bg fetch
            if self.bg_fifo.is_empty() {
                self.bg_fetch(scan_line, x_position_counter);
            }

            // sprite fetch

            // merge

            // push
            if x_position_counter == 0 && self.is_window_rendering(x_position_counter) {
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

    fn is_window_rendering(&self, x_coordinate: u8) -> bool {
        let lcd5 = self.read_lcd_bit(5);
        let is_window_line = self.wy == self.ly;
        let is_window_x_pos = x_coordinate + 7 >= self.wx;

        return lcd5 && is_window_line && is_window_x_pos;
    }
    

    pub fn render(&mut self, frame: &mut [u8]) -> Result<()> {
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let pixel_data = self.frame_buffer[i];
            pixel.copy_from_slice(&pixel_data);
        }
        Ok(())
    }

    pub fn dump(&self) {
        let start = 0x1000;
        for i in start..=0x1500 {
            let mut v = Vec::new();
            for j in 0..16 {
                v.push(self.vram[i+j]);
            }
            println!("{:X}: {:?}", i+0x8000, v);
        }
    }

    pub fn bg_fetch(&mut self, scan_line: u8, x_coordinate: u8) {
        let lcd4 = self.read_lcd_bit(4);
        let lcd3 = self.read_lcd_bit(3);

        // fetch tile number
        let bg_tile_map_address: u16 = if lcd3 { 0x1C00 } else { 0x1800 };

        let scx: u16 = self.scx as u16;
        let ly: u16 = scan_line as u16;
        let scy: u16 = self.scy as u16;
        // タイル番号の横幅は0~31までなので0x1f(31)でandする
        let x_offset = ((scx+x_coordinate as u16) / 8) & 0x1F;
        let y_offset = ((ly + scy) / 8) * 32;
        // tile_map_idx = ((scx + x_coordinate) / 8) + (((ly + scy) / 8) * 32)
        // タイル番号は32 * 32の0~1023なので0x3FF(1023)でandする
        let tile_map_idx = (x_offset + y_offset) & 0x3FF;
        // println!("tile_map_idx = {}", tile_map_idx);
        let tile_number_address: u16 = tile_map_idx + bg_tile_map_address as u16;
        // println!("tile_number_address = 0x{:X}", tile_number_address + 0x8000);
        let tile_number = self.vram[tile_number_address as usize];
        // println!("tile_number = {}", tile_number);

        // fetch tile data (low)
        let mut tile_address: usize = if lcd4 {
            tile_number as usize * 16 
        }
        else { 
            let signed_tile_number: i8 = tile_number as i8;
            (signed_tile_number as i16 * 16 + 0x1000) as usize
        };
        // println!("tile_address = 0x{:X}", tile_address+0x8000);
        let tile_vertical_offset = ((ly + scy) % 8) * 2;
        tile_address += tile_vertical_offset as usize;
        let lower_tile_data = self.vram[tile_address];

        // fetch tile data (high)
        let higher_tile_data = self.vram[tile_address + 1];

        // println!("tile_data = 0x{:02X} 0x{:02X}", lower_tile_data, higher_tile_data);

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

    pub fn window_fetch(&mut self, x_coordinate: u8) {
        let lcd4 = self.read_lcd_bit(6);
        let lcd3 = self.read_lcd_bit(3);

        // fetch tile number
        let window_tile_map_address: u16 = if lcd3 { 0x1C00 } else { 0x1800 };

        let window_x = x_coordinate.wrapping_sub(self.wx);

        // タイル番号の横幅は0~31までなので0x1f(31)でandする
        let x_offset = ((window_x as u16) / 8) & 0x1F;
        let y_offset = ((self.window_line_counter as u16) / 8) * 32;
        // tile_map_idx = (window_x / 8) + (window_line_counter / 8) * 32)
        // タイル番号は32 * 32の0~1023なので0x3FF(1023)でandする
        let tile_map_idx = (x_offset + y_offset) & 0x3FF;
        // println!("tile_map_idx = {}", tile_map_idx);
        let tile_number_address: u16 = tile_map_idx + window_tile_map_address as u16;
        // println!("tile_number_address = 0x{:X}", tile_number_address + 0x8000);
        let tile_number = self.vram[tile_number_address as usize];
        // println!("tile_number = {}", tile_number);

        // fetch tile data (low)
        let mut tile_address: usize = if lcd4 {
            tile_number as usize * 16 
        }
        else { 
            let signed_tile_number: i8 = tile_number as i8;
            (signed_tile_number as i16 * 16 + 0x1000) as usize
        };
        // println!("tile_address = 0x{:X}", tile_address+0x8000);
        let tile_vertical_offset = (self.window_line_counter % 8) * 2;
        tile_address += tile_vertical_offset as usize;
        let lower_tile_data = self.vram[tile_address];

        // fetch tile data (high)
        let higher_tile_data = self.vram[tile_address + 1];

        // println!("tile_data = 0x{:02X} 0x{:02X}", lower_tile_data, higher_tile_data);

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
