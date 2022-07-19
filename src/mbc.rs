use anyhow::{Result, bail};

use crate::rom::Rom;

pub trait Mbc {
    fn read_rom(&self, address: u16) -> Result<u8>;
    fn read_ram(&self, address: u16) -> Result<u8> {
        Ok(0)
    }
    fn write_rom(&mut self, address: u16, data: u8) -> Result<()> {
        Ok(())
    }
    fn write_ram(&mut self, address: u16, data: u8) -> Result<()> {
        Ok(())
    }
    fn write_registers(&mut self, address: u16, data: u8) -> Result<()> {
        Ok(())
    }
}

pub struct NoMbc {
    pub mbc_type: u8,
    pub rom: Rom
}

pub struct Mbc1 {
    pub mbc_type: u8,
    pub rom: Rom,
    pub rom_size: u8,
    pub ram_size: u8,
    pub is_external_ram_enable: bool,
    pub rom_bank_number: u8,
    pub ram_bank_number: u8,
    pub mode_flag: bool,
    pub ram: [u8; 0x8000]
}

impl Mbc for NoMbc {
    fn read_rom(&self, address: u16) -> Result<u8> {
        let ret = self.rom.data[address as usize];
        Ok(ret)
    }
}

impl Mbc for Mbc1 {
    fn read_rom(&self, raw_address: u16) -> Result<u8> {
        match raw_address {
            0x0000..=0x3FFF => {
                if !self.mode_flag {
                    Ok(self.rom.data[raw_address as usize])
                }
                else {
                    let zero_bank_number = match self.rom_size {
                        0x00..=0x04 => 0,
                        0x05 => (self.ram_bank_number & 0x01) << 5,
                        0x06 | _ => (self.ram_bank_number & 0x11) << 6
                    };

                    let address = 0x4000 * zero_bank_number as usize + raw_address as usize;
                    Ok(self.rom.data[address])
                }
            },
            0x4000..=0x7FFF => {
                let rom_size_mask_bit = match self.rom_size {
                    0..=6 => (1 << (self.rom_size + 1)) - 1,
                    _ => bail!("MBC1 does not support this rom size")
                };

                let high_bank_number = match self.rom_bank_number {
                    0 => 1,
                    _ => {
                        match self.rom_size {
                            // 512KBまでは書き込まれたデータをマスクするだけ
                            0..=4 => self.rom_bank_number & rom_size_mask_bit,
                            // 1MB
                            5 => (self.rom_bank_number & rom_size_mask_bit) + ((self.ram_bank_number & 0x01) << 5),
                            // 2MB
                            6 | _ => (self.rom_bank_number & rom_size_mask_bit) + ((self.ram_bank_number & 0x11) << 6)
                        }
                    }
                };

                let address = 0x4000 * high_bank_number as u16 + (raw_address - 0x4000);
                let ret = self.rom.data[address as usize];
                Ok(ret)
            },
            _ => bail!("Error in mbc1: invalid address")
        }
    }

    fn read_ram(&self, raw_address: u16) -> Result<u8> {
        if self.is_external_ram_enable {
            let data = match self.ram_size {
                0x02 => {
                    let address = raw_address as usize - 0xA000_usize;
                    self.ram[address]
                },
                0x03 => {
                    if self.mode_flag {
                        let address = (0x2000 * self.ram_bank_number as usize) + (raw_address as usize - 0xA000);
                        self.ram[address]
                    }
                    else {
                        let address = raw_address as usize - 0xA000;
                        self.ram[address]
                    }
                },
                _ => bail!("Error in mbc1: invalid ram size")
            };

            Ok(data)
        }
        else {
            Ok(0xFF)
        }
    }

    fn write_ram(&mut self, raw_address: u16, data: u8) -> Result<()> {
        if self.is_external_ram_enable {
            match self.ram_size {
                0x02 => {
                    let address = raw_address as usize - 0xA000_usize;
                    self.ram[address] = data;
                },
                0x03 => {
                    if self.mode_flag {
                        let address = (0x2000 * self.ram_bank_number as usize) + (raw_address as usize - 0xA000);
                        self.ram[address] = data;
                    }
                    else {
                        let address = raw_address as usize - 0xA000;
                        self.ram[address] = data;
                    }
                },
                _ => bail!("Error in mbc1: invalid ram size")
            };

            Ok(())
        }
        else {

            Ok(())
        }
    }

    fn write_registers(&mut self, address: u16, data: u8) -> Result<()> {
        match address {
            0x0000..=0x1FFF => {
                if data == 0x0A {
                    self.is_external_ram_enable = true;
                }
                else {
                    self.is_external_ram_enable = false;
                }
            },
            0x2000..=0x3FFF => self.rom_bank_number = data,
            0x4000..=0x5FFF => self.ram_bank_number = data & 0x11,
            0x6000..=0x7FFF => self.mode_flag = (data & 0x01) == 0x01,
            _ => bail!("Error in mbc1: Invalid address")
        }
        Ok(())
    }
}
