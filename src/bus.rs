use std::{io::BufReader, fs::File};

use anyhow::{Result, bail};

use crate::{mbc::{Mbc, NoMbc, Mbc1}, ppu::Ppu, joypad::Joypad, timer::Timer, rom::{Rom}};

pub struct Bus {
    pub ram: [u8; 0x8192],
    pub hram: [u8; 0x127],
    pub ppu: Ppu,
    pub mbc: Box<dyn Mbc>,
    pub dma: u8,
    pub timer: Timer,
    pub joypad: Joypad,
    // interrupt enable
    pub ie_flag: u8,
    // interrupt flag
    pub int_flag: u8
}

impl Bus {
    pub fn new(reader: &mut BufReader<File>) -> Self {
        let rom = Rom::new(reader).unwrap();
        let rom_type = rom.cartridge_type;
        let rom_size = rom.rom_size;
        let ram_size = rom.ram_size;

        let mbc: Box<dyn Mbc> = match rom_type {
            0x00 => {
                Box::new(
                    NoMbc {
                        rom,
                        mbc_type: rom_type,
                    }
                )
            },
            0x01 | 0x02 | 0x03 | _ => {
                Box::new(
                    Mbc1 {
                        rom,
                        mbc_type: rom_type,
                        rom_size: rom_size,
                        ram_size: ram_size,
                        is_external_ram_enable: Default::default(),
                        rom_bank_number: Default::default(),
                        ram_bank_number: Default::default(),
                        mode_flag: Default::default(),
                        ram: [0; 0x8000]
                    }
                )
            }
        };

        let ppu = Ppu::new();

        Self { 
            ram: [0; 0x8192],
            hram: [0; 0x127],
            ppu,
            mbc,
            timer: Default::default(),
            dma: Default::default(),
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
            0xE000..=0xFDFF => Ok(self.ram[(address-0xE000) as usize]),
            0xFE00..=0xFE9F => self.ppu.read_OAM(address-0xFE00),
            0xFEA0..=0xFEFF => Ok(0),
            0xFF00 => Ok(self.joypad.read()),
            0xFF01..=0xFF03 => Ok(0),
            0xFF04 => Ok(self.timer.read_div()),
            0xFF05 => Ok(self.timer.read_tima()),
            0xFF06 => Ok(self.timer.read_tma()),
            0xFF07 => Ok(self.timer.read_tac()),
            0xFF10..=0xFF3F => Ok(0),
            0xFF40 => self.ppu.lcd_control_read(),
            0xFF41 => self.ppu.read_lcd_stat(),
            0xFF42 => self.ppu.scy_read(),
            0xFF43 => self.ppu.scx_read(),
            0xFF44 => self.ppu.ly_read(),
            0xFF45 => self.ppu.lyc_read(),
            0xFF46 => Ok(self.dma),
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
            0xE000..=0xFDFF => {
                self.ram[(address-0xE000) as usize] = data;
                Ok(())
            },
            0xFE00..=0xFE9F => self.ppu.write_OAM(address-0xFE00, data, false),
            0xFEA0..=0xFEFF => Ok(()),
            0xFF00 => {
                self.joypad.write(data);
                Ok(())
            },
            0xFF01..=0xFF03 => Ok(()),
            0xFF04 => {
                self.timer.write_div(data);
                Ok(())
            },
            0xFF05 => {
                self.timer.write_tima(data);
                Ok(())
            },
            0xFF06 => {
                self.timer.write_tma(data);
                Ok(())
            },
            0xFF07 => {
                self.timer.write_tac(data);
                Ok(())
            },
            0xFF0F => {
                self.int_flag = data;
                Ok(())
            },
            0xFF10..=0xFF3F => Ok(()),
            0xFF40 => self.ppu.lcd_control_write(data),
            0xFF41 => self.ppu.write_lcd_stat(data),
            0xFF42 => self.ppu.scy_write(data),
            0xFF43 => self.ppu.scx_write(data),
            0xFF44 => self.ppu.ly_write(data),
            0xFF45 => self.ppu.lyc_write(data),
            0xFF46 => self.excute_dma(data),
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

    fn excute_dma(&mut self, data: u8) -> Result<()> {
        self.dma = data;
        let source: u16 = (data as u16) << 8;
        for i in 0..0xA0_u16 {
            let src_address = source + i;
            let data = self.read(src_address)?;
            let dest_address = i;
            self.ppu.write_OAM(dest_address, data, true)?;
        }

        Ok(())
    }
}
