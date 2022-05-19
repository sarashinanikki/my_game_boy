use anyhow::Result;

pub enum Mode {
    OamScan,
    Drawing,
    HBlank,
    VBlank
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
    mode: Mode
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            vram: [0; 0x8192],
            lcd_control: Default::default(),
            scy: Default::default(),
            scx: Default::default(),
            ly: Default::default(),
            lyc: Default::default(),
            wy: Default::default(),
            wx: Default::default(),
            mode: Default::default()
        }
    }

    pub fn render(&mut self, frame: &mut [u8]) -> Result<()> {
        let lcd0 = self.read_lcd_bit(0);
        let lcd4 = self.read_lcd_bit(4);
        let lcd6 = self.read_lcd_bit(6);

        let mut background: [u8; 256 * 256] = [0; 256 * 256];

        let black: [u8; 4] = [0x00, 0x00, 0x00, 0xff];
        let white: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
        let dark_gray: [u8; 4] = [0x1e, 0x51, 0x28, 0xff];
        let light_gray: [u8; 4] = [0xd8, 0xe9, 0xa8, 0xff];

        let color_palette: [[u8; 4]; 4] = [white, light_gray, dark_gray, black];

        // BG Draw
        if lcd0 {
            self.bg_draw(lcd6, lcd4, &mut background);
        }

        let start_index = self.scy * 32 + self.scx;
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let idx = (i + start_index as usize) % (256 * 256);
            let pixel_data = color_palette[background[idx] as usize];
            pixel.copy_from_slice(&pixel_data);
        }

        Ok(())
    }

    pub fn bg_draw(&mut self, lcd6: bool, lcd4: bool, background: &mut [u8; 256 * 256]) {
        let start_address: u16 = if lcd6 { 0x1800 } else { 0x1C00 };

        for i in 0_usize..32_usize {
            for j in 0_usize..32_usize {
                let tile_number_idx = i*32 + j;
                let tile_number_address: usize = tile_number_idx + start_address as usize;
                let tile_number = self.vram[tile_number_address];
                let tile_address: usize = if lcd4 { tile_number as usize * 16 } else { (0x1000_i16 + (tile_number as i16)) as usize };

                let index_y = i * 32 * 8;
                let index_x = j * 8;

                for k in (0..16).step_by(2) {
                    let lower_byte = self.vram[tile_address+k];
                    let upper_byte = self.vram[tile_address+k+1];

                    for bit in 0..8 {
                        let top = if upper_byte & (1 << bit) == (1 << bit) {1_u8} else {0_u8};
                        let bottom = if lower_byte & (1 << bit) == (1 << bit) {1_u8} else {0_u8};

                        let pixel_data = top*2 + bottom;
                        
                        let background_index = index_y + (k / 2) + index_x + (7 - bit);
                        background[background_index] = pixel_data;
                    }
                }
            }
        }

        return;
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

    fn read_lcd_bit(&self, bit: u8) -> bool {
        return &self.lcd_control & (1 << bit) == (1 << bit);
    }
}
