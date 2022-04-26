use anyhow::Result;

pub struct Ppu {
    vram: [u8; 0x8192],
    lcd_control: u8,
    scy: u8,
    scx: u8,
    ly: u8,
    lyc: u8,
    wy: u8,
    wx: u8
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
            wx: Default::default()
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

    pub fn lcd_control_write(&mut self, address: u16, data: u8) -> Result<()> {
        self.lcd_control = data;
        Ok(())
    }

    pub fn lcd_control_read(&self, address: u16) -> Result<u8> {
        let data = self.lcd_control;
        Ok(data)
    }

    pub fn scy_write(&mut self, address: u16, data: u8) -> Result<()> {
        self.scy = data;
        Ok(())
    }

    pub fn scy_control_read(&self, address: u16) -> Result<u8> {
        let data = self.scy;
        Ok(data)
    }

    pub fn ly_write(&mut self, address: u16, data: u8) -> Result<()> {
        self.ly = data;
        Ok(())
    }

    pub fn ly_read(&self, address: u16) -> Result<u8> {
        let data = self.ly;
        Ok(data)
    }

    pub fn lyc_write(&mut self, address: u16, data: u8) -> Result<()> {
        self.lyc = data;
        Ok(())
    }

    pub fn lyc_read(&self, address: u16) -> Result<u8> {
        let data = self.lyc;
        Ok(data)
    }

    pub fn wy_write(&mut self, address: u16, data: u8) -> Result<()> {
        self.wy = data;
        Ok(())
    }

    pub fn wy_read(&self, address: u16) -> Result<u8> {
        let data = self.wy;
        Ok(data)
    }

    pub fn wx_write(&mut self, address: u16, data: u8) -> Result<()> {
        self.wx = data;
        Ok(())
    }

    pub fn wx_read(&self, address: u16) -> Result<u8> {
        let data = self.wx;
        Ok(data)
    }
}
