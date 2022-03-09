use anyhow::{Result, bail};

use crate::mbc::Mbc;

pub struct Bus {
    pub ram: [u8; 0x8000],
    pub hram: [u8; 0x127],
    pub mbc: Box<dyn Mbc>
}

impl Bus {
    fn new(mbc: Box<dyn Mbc>) -> Self {
        Self { ram: [0; 0x8000], hram: [0; 0x127], mbc }
    }

    fn read(&self, address: u16) -> Result<u8> {
        match address {
            0x0000..=0x7FFF => self.mbc.read_rom(address),
            // 0x8000..=0x9FFF => VRAM,
            0xA000..=0xBFFF => self.mbc.read_ram(address),
            0xC000..=0xDFFF => Ok(self.ram[(address-0xC000) as usize]),
            // 0xE000..=0xFDFF => ECHO RAM,
            // 0xFE00..=0xFE9F => OAM,
            0xFEA0..=0xFEFF => Ok(0),
            // 0xFF00..=0xFF7F => IO,
            0xFF80..=0xFFFE => Ok(self.hram[(address-0xFF80) as usize]),
            // 0xFFFF => IE,
            _ => bail!("fail! invalid address")
        }
    }

    fn write(&mut self, address: u16, data: u8) -> Result<()> {
        match address {
            0x0000..=0x7FFF => self.mbc.write_rom(address, data),
            // 0x8000..=0x9FFF => VRAM,
            0xA000..=0xBFFF => self.mbc.write_ram(address, data),
            0xC000..=0xDFFF => {
                self.ram[(address-0xC000) as usize] = data;
                Ok(())
            },
            // 0xE000..=0xFDFF => ECHO RAM,
            // 0xFE00..=0xFE9F => OAM,
            0xFEA0..=0xFEFF => Ok(()),
            // 0xFF00..=0xFF7F => IO,
            0xFF80..=0xFFFE => {
                self.hram[(address-0xFF80) as usize] = data;
                Ok(())
            }
            // 0xFFFF => IE,
            _ => bail!("fail! invalid address")
        }
    }
}
