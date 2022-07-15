use anyhow::{Result, bail};

use crate::{mbc::Mbc, ppu::Ppu, joypad::{Joypad,Button}};

pub struct Bus {
    pub ram: [u8; 0x8192],
    pub hram: [u8; 0x127],
    pub ppu: Ppu,
    pub mbc: Box<dyn Mbc>,
    pub joypad: Joypad,
    // interrupt enable
    pub ie_flag: u8,
    // interrupt flag
    pub int_flag: u8
}

impl Bus {
    pub fn new(mbc: Box<dyn Mbc>, ppu: Ppu) -> Self {
        Self { 
            ram: [0; 0x8192],
            hram: [0; 0x127],
            ppu,
            mbc,
            joypad: Default::default(),
            ie_flag: Default::default(),
            int_flag: Default::default()
        }
    }

    pub fn read(&self, address: u16) -> Result<u8> {
        match address {
            0x0000..=0x7FFF => self.mbc.read_rom(address),
            0x8000..=0x9FFF => self.ppu.read(address-0x8000),
            0xA000..=0xBFFF => self.mbc.read_ram(address-0xA000),
            0xC000..=0xDFFF => Ok(self.ram[(address-0xC000) as usize]),
            // 0xE000..=0xFDFF => ECHO RAM,
            0xFE00..=0xFE9F => self.ppu.read_OAM(address-0xFE00),
            0xFEA0..=0xFEFF => Ok(0),
            0xFF00 => Ok(self.joypad.read()),
            0xFF01 => self.ppu.read_lcd_stat(),
            // 0xFF02..=0xFF7F => IO,
            0xFF26 => Ok(0),
            0xFF40 => self.ppu.lcd_control_read(),
            0xFF42 => self.ppu.scy_read(),
            0xFF43 => self.ppu.scx_read(),
            0xFF44 => self.ppu.ly_read(),
            0xFF45 => self.ppu.lyc_read(),
            0xFF47 => self.ppu.bgp_read(),
            0xFF48..=0xFF49 => self.ppu.read_obp(address),
            0xFF4A => self.ppu.wy_read(),
            0xFF4B => self.ppu.wx_read(),
            0xFF0F => Ok(self.int_flag),
            0xFF80..=0xFFFE => Ok(self.hram[(address-0xFF80) as usize]),
            0xFFFF => Ok(self.ie_flag),
            _ => bail!("fail! invalid address")
        }
    }

    pub fn read_16(&self, address: u16) -> Result<u16> {
        let low: u8 = self.read(address)?;
        let high: u8 = self.read(address+1)?;
        let data: u16 = ((high as u16) << 8) + low as u16;

        Ok(data)
    }

    pub fn write(&mut self, address: u16, data: u8) -> Result<()> {
        match address {
            0x0000..=0x7FFF => self.mbc.write_rom(address, data),
            0x8000..=0x9FFF => self.ppu.write(address-0x8000, data),
            0xA000..=0xBFFF => self.mbc.write_ram(address-0xA000, data),
            0xC000..=0xDFFF => {
                self.ram[(address-0xC000) as usize] = data;
                Ok(())
            },
            // 0xE000..=0xFDFF => ECHO RAM,
            0xFE00..=0xFE9F => self.ppu.write_OAM(address-0xFE00, data),
            0xFEA0..=0xFEFF => Ok(()),
            0xFF00 => {
                self.joypad.write(data);
                Ok(())
            }
            0xFF01 => self.ppu.write_lcd_stat(data),
            // 0xFF02..=0xFF7F => IO,
            0xFF0F => {
                self.int_flag = data;
                Ok(())
            }
            0xFF26 => Ok(()),
            0xFF40 => self.ppu.lcd_control_write(data),
            0xFF42 => self.ppu.scy_write(data),
            0xFF43 => self.ppu.scx_write(data),
            0xFF44 => self.ppu.ly_write(data),
            0xFF45 => self.ppu.lyc_write(data),
            0xFF47 => self.ppu.bgp_write(data),
            0xFF48..=0xFF49 => self.ppu.write_obp(address, data),
            0xFF4A => self.ppu.wy_write(data),
            0xFF4B => self.ppu.wx_write(data),
            0xFF80..=0xFFFE => {
                self.hram[(address-0xFF80) as usize] = data;
                Ok(())
            }
            0xFFFF => {
                self.ie_flag = data;
                Ok(())
            }
            _ => bail!("fail! invalid address")
        }
    }

    pub fn write_16(&mut self, address: u16, data: u16) -> Result<()> {
        let low: u8 = (data & 0x00FF) as u8;
        let high: u8 = (data >> 8) as u8;

        self.write(address, low)?;
        self.write(address+1, high)?;

        Ok(())
    }
}
