use anyhow::Result;

use crate::rom::Rom;

pub trait Mbc {
    fn read_rom(&self, address: u16) -> Result<u8>;
    fn read_ram(&self, address: u16) -> Result<u8>;
    fn write_rom(&mut self, address: u16, data: u8) -> Result<()>;
    fn write_ram(&mut self, address: u16, data: u8) -> Result<()>;
}

pub struct NoMbc {
    pub mbc_type: u8,
    pub rom: Rom
}

impl NoMbc {
    fn new(rom: Rom) -> Self {
        let mbc_type: u8 = 0;
        Self { mbc_type, rom }
    }
}

impl Mbc for NoMbc {
    fn read_rom(&self, address: u16) -> Result<u8> {
        let ret = self.rom.data[address as usize];
        Ok(ret)
    }

    fn read_ram(&self, _address: u16) -> Result<u8> {
        Ok(0)
    }

    fn write_rom(&mut self, _address: u16, _data: u8) -> Result<()> {
        Ok(())
    }

    fn write_ram(&mut self, _address: u16, _data: u8) -> Result<()> {
        Ok(())
    }
}
