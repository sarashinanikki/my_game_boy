use anyhow::Result;
use std::{collections::VecDeque, cmp::Ordering};

pub enum Mode {
    OamScan,
    Drawing,
    HBlank,
    VBlank
}

#[derive(Default, Clone, Copy, Debug)]
pub struct OAM {
    y_position: u8,
    x_position: u8,
    tile_number: u8,
    sprite_flags: u8
}

impl OAM {
    fn get(&self, address: u16) -> u8 {
        let idx = address % 4;
        match idx {
            0 => self.y_position,
            1 => self.x_position,
            2 => self.tile_number,
            _ => self.sprite_flags
        }
    }

    fn set(&mut self, address: u16, data: u8) {
        let idx = address % 4;
        match idx {
            0 => self.y_position = data,
            1 => self.x_position = data,
            2 => self.tile_number = data,
            _ => self.sprite_flags = data
        }
    }
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

pub enum Color {
    White,
    LightGray,
    DarkGray,
    Black
}

pub struct Palette([Color; 4]);

impl Default for Palette {
    fn default() -> Self {
        Self([Color::White, Color::LightGray, Color::DarkGray, Color::Black])
    }
}

pub struct Ppu {
    vram: [u8; 0x8192],
    oam: [OAM; 40],
    lcd_control: u8,
    scy: u8,
    scx: u8,
    ly: u8,
    lyc: u8,
    wy: u8,
    wx: u8,
    bgp: u8,
    obp: [u8; 2],
    window_line_counter: u8,
    bg_fifo: VecDeque<PixelData>,
    sprite_fifo: VecDeque<PixelData>,
    sprite_buffer: Vec<(OAM, usize)>,
    bg_color_palette: Palette,
    obp_color_palette: [Palette; 2],
    frame_buffer: [[u8; 4]; 160 * 144],
    current_cycle: usize,
    mode: Mode
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            vram: [0; 0x8192],
            oam: [OAM::default(); 40],
            lcd_control: 0x80,
            scy: Default::default(),
            scx: Default::default(),
            ly: Default::default(),
            lyc: Default::default(),
            wy: Default::default(),
            wx: Default::default(),
            bgp: Default::default(),
            obp: Default::default(),
            window_line_counter: Default::default(),
            bg_fifo: VecDeque::new(),
            sprite_fifo: VecDeque::new(),
            sprite_buffer: Vec::new(),
            bg_color_palette: Default::default(),
            obp_color_palette: Default::default(),
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
                    self.oam_scan();
                    self.assign_bg_palette();
                    self.assign_sprite_palette();
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

    fn oam_scan(&mut self) {
        self.sprite_buffer.clear();
        let sprite_height = if self.read_lcd_bit(2) { 16u8 } else { 8u8 };
        for (i, sp) in self.oam.iter().enumerate() {
            if sp.x_position > 0 && self.ly + 16 >= sp.y_position &&
                self.ly + 16 < sp.y_position + sprite_height && self.sprite_buffer.len() < 10
            {
                self.sprite_buffer.push((sp.clone(), i));
            }
        }

        // Non-CGB Modeにおいて、優先順位はX座標が小さい順であり、X座標が同じ場合はOAMメモリ上でより早く登場する順となる
        // TODO: CGB ModeではX座標関係なくメモリ順なので、将来的には処理を分ける
        self.sprite_buffer.sort_by(|a, b| {
                match a.0.x_position.cmp(&b.0.x_position) {
                    Ordering::Equal => a.1.cmp(&b.1),
                    Ordering::Less => Ordering::Less,
                    Ordering::Greater => Ordering::Greater
                }
            }
        );
    }

    fn fetch(&mut self) {
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
            if self.sprite_fifo.is_empty() {
                self.oam_fetch(x_position_counter);
            }

            // merge

            // push
            if x_position_counter == 0 && self.is_window_rendering(x_position_counter) {
                let discard = self.scx % 8;
                for _ in 0..discard {
                    self.bg_fifo.pop_front();
                }
            }


            let bg_color_idx = self.bg_fifo.pop_front().unwrap().color;
            let sprite_pixel = self.sprite_fifo.pop_front().unwrap_or(PixelData{
                color: 0,
                background_priority: 0,
                palette: 0,
                sprite_priority: 0
            });

            let color = match sprite_pixel.color {
                0 => self.apply_bg_pixel_color(bg_color_idx),
                _ => {
                    if sprite_pixel.sprite_priority > 0 && bg_color_idx != 0 {
                        self.apply_bg_pixel_color(bg_color_idx)
                    }
                    else {
                        let sp_color_idx = sprite_pixel.color;
                        let palette = sprite_pixel.palette;
                        self.apply_sprite_pixel_color(sp_color_idx, palette)
                    }
                }
            };

            for i in 0..4 {
                self.frame_buffer[(scan_line as usize * 160 + x_position_counter as usize) as usize][i] = color[i];
            }
        }

    }

    fn assign_bg_palette(&mut self) {
        for i in 0..4 {
            let bit = i * 2;
            let lower_bit = if (self.bgp & (1 << bit)) == (1 << bit) { 1_u8 } else { 0_u8 };
            let higher_bit = if (self.bgp & 1 << (bit+1)) == (1 << (bit+1)) { 2_u8 } else { 0_u8 };
            let color_val = higher_bit + lower_bit;
            
            let color = match color_val {
                0 => Color::White,
                1 => Color::LightGray,
                2 => Color::DarkGray,
                _ => Color::Black
            };
            
            self.bg_color_palette.0[i] = color;
        }
    }

    fn assign_sprite_palette(&mut self) {
        for i in 0..2 {
            for j in 0..4 {
                let bit = i * 2;
                let lower_bit = if (self.obp[i] & (1 << bit)) == (1 << bit) { 1_u8 } else { 0_u8 };
                let higher_bit = if (self.obp[i] & 1 << (bit+1)) == (1 << (bit+1)) { 2_u8 } else { 0_u8 };
                let color_val = higher_bit + lower_bit;
                
                let color = match color_val {
                    0 => Color::White,
                    1 => Color::LightGray,
                    2 => Color::DarkGray,
                    _ => Color::Black
                };
                
                self.obp_color_palette[i].0[j] = color;
            }
        }
    }

    fn apply_bg_pixel_color(&self, color_idx: u8) -> [u8; 4] {
        let black: [u8; 4] = [0x00, 0x00, 0x00, 0xff];
        let white: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
        let dark_gray: [u8; 4] = [0x1e, 0x51, 0x28, 0xff];
        let light_gray: [u8; 4] = [0xd8, 0xe9, 0xa8, 0xff];

        let color = match self.bg_color_palette.0[color_idx as usize] {
            Color::White => white,
            Color::LightGray => light_gray,
            Color::DarkGray => dark_gray,
            Color::Black => black
        };

        return color;
    }

    fn apply_sprite_pixel_color(&self, color_idx: u8, palette: u8) -> [u8; 4] {
        let black: [u8; 4] = [0x00, 0x00, 0x00, 0xff];
        let white: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
        let dark_gray: [u8; 4] = [0x1e, 0x51, 0x28, 0xff];
        let light_gray: [u8; 4] = [0xd8, 0xe9, 0xa8, 0xff];

        let color = match self.obp_color_palette[palette as usize].0[color_idx as usize] {
            Color::White => white,
            Color::LightGray => light_gray,
            Color::DarkGray => dark_gray,
            Color::Black => black
        };

        return color;
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

    fn oam_fetch(&mut self, x_coordinate: u8) {
        let target_sprite = self.sprite_buffer.iter().find(|el| {
            el.0.x_position <= x_coordinate + 8 && x_coordinate <= el.0.x_position
        });

        if let Some(target) = target_sprite {
            // 必要な変数の準備
            let y_position = target.0.y_position;
            let x_position = target.0.x_position;
            let tile_number = target.0.tile_number;
            let sprite_flags = target.0.sprite_flags;

            let palette_number = if sprite_flags & (1 << 4) == (1 << 4) { 1u8 } else { 0u8 };
            let x_flip = sprite_flags & (1 << 5) == (1 << 5);
            let y_flip = sprite_flags & (1 << 6) == (1 << 6);
            let priority = if sprite_flags & (1 << 7) == (1 << 7) { 1_u8 } else { 0_u8 };
            let scan_line = self.ly;

            let sprite_size = if self.read_lcd_bit(2) { 16u8 } else { 8u8 };
            let x_limit = x_position.wrapping_sub(x_coordinate);
            
            // spriteのサイズで場合分け
            if sprite_size == 8 {
                let mut tile_vertical_offset = 2 * (((scan_line + 16).wrapping_sub(y_position)) % 8);
                
                // vertical_offsetは0,2,4,6,8,10,12,14のどれかになる
                // 上下反転していればこれも逆になるので、14-vertical_offsetとする
                if y_flip {
                    tile_vertical_offset = 14 - tile_vertical_offset;
                }
                
                let tile_address = (tile_number * 16 + tile_vertical_offset) as usize;

                let lower_tile_data = self.vram[tile_address];
                let higher_tile_data = self.vram[tile_address + 1];

                for i in 0..8_u8 {
                    let bit = if x_flip { 7-i } else { i };
                    let top = if higher_tile_data & (1 << bit) == (1 << bit) {1_u8} else {0_u8};
                    let bottom = if lower_tile_data & (1 << bit) == (1 << bit) {1_u8} else {0_u8};

                    let pixel_color = top*2 + bottom;
                    let pixel_data = PixelData {
                        color: pixel_color,
                        background_priority: priority,
                        palette: palette_number,
                        sprite_priority: 0 // only relevant for CGB
                    };

                    self.sprite_fifo.push_back(pixel_data);

                    if self.sprite_fifo.len() >= x_limit as usize {
                        break;
                    }
                }
            }
            else {
                let mut tile_vertical_offset = 2 * (((scan_line + 16).wrapping_sub(y_position)) % 16);

                // vertical_offsetは0,2,4,6,8,10,12,14,16,18,20,22,24,26,28,30のどれかになる
                // 上下反転していればこれも逆になるので、30-vertical_offsetとする
                if y_flip {
                    tile_vertical_offset = 30 - tile_vertical_offset;
                }

                // 上側のタイルはタイル番号 & 0xFE
                // 下側のタイルはタイル番号 | 0x01
                // offsetが16以上 -> 下側のタイル
                let tile_address = if tile_vertical_offset >= 16 {
                    let bottom_tile_number = tile_number | 0x01;
                    (bottom_tile_number * 16 + (tile_vertical_offset - 16)) as usize
                }
                // offsetが16未満 -> 上側のタイル
                else {
                    let top_tile_number = tile_number & 0xFE;
                    (top_tile_number * 16 + tile_vertical_offset) as usize
                };

                let lower_tile_data = self.vram[tile_address];
                let higher_tile_data = self.vram[tile_address + 1];

                for i in 0..8_u8 {
                    let bit = if x_flip { 7-i } else { i };
                    let top = if higher_tile_data & (1 << bit) == (1 << bit) {1_u8} else {0_u8};
                    let bottom = if lower_tile_data & (1 << bit) == (1 << bit) {1_u8} else {0_u8};

                    let pixel_color = top*2 + bottom;
                    let pixel_data = PixelData {
                        color: pixel_color,
                        background_priority: priority,
                        palette: palette_number,
                        sprite_priority: 0 // only relevant for CGB
                    };

                    self.sprite_fifo.push_back(pixel_data);

                    if self.sprite_fifo.len() >= x_limit as usize {
                        break;
                    }
                }
            }

        }
        
    }

    fn bg_fetch(&mut self, scan_line: u8, x_coordinate: u8) {
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

    fn window_fetch(&mut self, x_coordinate: u8) {
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

    pub fn write_OAM(&mut self, address: u16, data: u8) -> Result<()> {
        self.oam[address as usize].set(address, data);
        Ok(())
    }

    pub fn read_OAM(&self,address: u16) -> Result<u8> {
        let data = self.oam[address as usize].get(address);
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

    pub fn read_obp(&self, address: u16) -> Result<u8> {
        let data = self.obp[(address - 0xFF48) as usize];
        Ok(data)
    }

    pub fn write_obp(&mut self, address: u16, data: u8) -> Result<()> {
        self.obp[(address - 0xFF48) as usize] = data;
        Ok(())
    }

    fn read_lcd_bit(&self, bit: u8) -> bool {
        return &self.lcd_control & (1 << bit) == (1 << bit);
    }
}
