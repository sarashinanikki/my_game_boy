use anyhow::{bail, Result};
use std::io::{BufReader, Seek, SeekFrom, Read};
use std::fs::File;

pub enum CGBMode {
    NotOnlyCGB,
    OnlyCGB
}

impl Default for CGBMode {
    fn default() -> Self {
        CGBMode::NotOnlyCGB
    }
}

pub enum DestinationCode {
    Japanese,
    NonJapanese
}

impl Default for DestinationCode {
    fn default() -> Self {
        DestinationCode::Japanese
    }
}

pub struct Rom {
    pub entry_point: [u8; 4],
    pub logo: [u8; 0x0030],
    pub title: [u8; 0x0010],
    pub manufacturer_code: [u8; 4],
    pub cgb_flag: CGBMode,
    pub licensee_code: [u8; 2],
    pub sgb_flag: bool,
    pub cartridge_type: u8,
    pub rom_size: usize,
    pub ram_size: usize,
    pub destination_code: DestinationCode,
    pub old_licensee_code: u8,
    pub version: u8,
    pub header_check_sum: u8,
    pub global_check_sum: [u8; 2],
    pub data: Vec<u8>,
}

impl Default for Rom {
    fn default() -> Self {
        Rom {
            entry_point: Default::default(),
            logo: [0; 0x0030],
            title: Default::default(),
            manufacturer_code: Default::default(),
            cgb_flag: Default::default(),
            licensee_code: Default::default(),
            sgb_flag: Default::default(),
            cartridge_type: Default::default(),
            rom_size: Default::default(),
            ram_size: Default::default(),
            destination_code: Default::default(),
            old_licensee_code: Default::default(),
            version: Default::default(),
            header_check_sum: Default::default(),
            global_check_sum: Default::default(),
            data: Vec::new(),
        }
    }
}

impl Rom {
    pub fn new(reader: &mut BufReader<File>) -> Result<Rom> {
        let mut rom: Rom = Default::default();

        // 0x100から読み込んでいく
        reader.seek(SeekFrom::Start(0x100))?;

        // ヘッダ読み込み
        reader.read_exact(&mut rom.entry_point[..])?;
        reader.read_exact(&mut rom.logo[..])?;
        reader.read_exact(&mut rom.title[..])?;

        rom.manufacturer_code.copy_from_slice(&rom.title[11..15]);

        // CGBのみの対応かどうか
        rom.cgb_flag = match rom.title.last() {
            // OnlyCGB
            Some(0xC0) => CGBMode::OnlyCGB,
            // not only CGB
            Some(0x80) => CGBMode::NotOnlyCGB,
            Some(_unknown) => CGBMode::NotOnlyCGB,
            _ => bail!("fail! GCBFlag is broken or there is unexpected EOF in CGB flag")
        };

        // licensee codeの読み込み
        reader.read_exact(&mut rom.licensee_code[..])?;

        // SGBの対応についての読み込み
        rom.sgb_flag = match reader.take(1).bytes().next() {
            Some(Ok(0x03)) => true,
            Some(Ok(0x00)) => false,
            Some(Ok(_unknown)) => false,
            _ => bail!("fail! SGBFlag is broken or there is unexpected EOF in SGB flag")
        };

        // MBCのタイプについての読み込み。TODO: そのうちここもEnumにする？
        rom.cartridge_type = match reader.take(1).bytes().next() {
            Some(Ok(res)) => res,
            Some(Err(_err)) => bail!("fail! a byte data of cartridge type is broken"),
            None => bail!("fail! There is unexpected EOF in cartridge type")
        };

        // 資料を見ながらROM sizeを計算する
        rom.rom_size = match reader.take(1).bytes().next() {
            Some(Ok(res @ 0x00..=0x08)) => (1 << res) * 32 * 1024_usize,
            Some(Ok(0x52)) => (11 * 1024 * 1024_usize) / 10_usize,
            Some(Ok(0x53)) => (12 * 1024 * 1024_usize) / 10_usize,
            Some(Ok(0x54)) => (15 * 1024 * 1024_usize) / 10_usize,
            Some(Ok(_unknown)) => bail!("fail! Unknown data in Rom size, actual data is {}", _unknown),
            Some(Err(_err)) => bail!("fail! a byte data of ROM size is broken"),
            None => bail!("fail! There is unexpected EOF in ROM size")
        };

        // 資料を見ながらRAM sizeを計算する
        rom.ram_size = match reader.take(1).bytes().next() {
            Some(Ok(0 | 1)) => 0,
            Some(Ok(res @ 2..=5)) => (1 << res) * 1024_usize,
            Some(Ok(_unknown)) => bail!("fail! unknown data in RAM size"),
            Some(Err(_err)) => bail!("fail! a byte data of RAM size is broken"),
            None => bail!("fail! There is unexpected EOF in RAM size")
        };

        // destination codeを読み込む
        rom.destination_code = match reader.take(1).bytes().next() {
            Some(Ok(0x00)) => DestinationCode::Japanese,
            Some(Ok(0x01)) => DestinationCode::NonJapanese,
            Some(Ok(_unknown )) => bail!("fail! a byte data of destination code is broken"),
            Some(Err(_err)) => bail!("fail! a byte data of destination code is broken"),
            None => bail!("fail! There is unexpected EOF in destination code")
        };

        // old licensee codeを読み込む
        rom.old_licensee_code = match reader.take(1).bytes().next() {
            Some(Ok(res)) => res,
            Some(Err(_err)) => bail!("fail! a byte data of old licensee code is broken"),
            None => bail!("fail! There is unexpected EOF in old licensee code")
        };

        // ROMのversion情報を読み込む
        rom.version = match reader.take(1).bytes().next() {
            Some(Ok(res)) => res,
            Some(Err(_err)) => bail!("fail! a byte data of version number is broken"),
            None => bail!("fail! There is unexpected EOF in version number")
        };

        // headerのチェックサムを読み込む
        rom.header_check_sum = match reader.take(1).bytes().next() {
            Some(Ok(res)) => res,
            Some(Err(_err)) => bail!("fail! a byte data of version number is header checksum"),
            None => bail!("fail! There is unexpected EOF in header checksum")
        };

        // globalなチェックサムを読み込む。Game Boy実機ではこのチェックサムの確認は実装されていないらしい。
        reader.read_exact(&mut rom.global_check_sum[..])?;

        // headerのチェックサムを計算する
        let mut header_checksum: u8 = 0;
        reader.seek(SeekFrom::Start(0x134))?;

        for _ in 0x134..=0x14C {
            let v = match reader.take(1).bytes().next() {
                Some(Ok(res)) => res,
                _ => bail!("fail! Some error occured in calculating checksum")
            };

            header_checksum = header_checksum.wrapping_sub(v).wrapping_sub(1);
        }

        if header_checksum != rom.header_check_sum {
            bail!("Actual checksum is different from header checksum which is in ROM");
        }

        // Read All Data
        reader.seek(SeekFrom::Start(0x000))?;
        reader.read_to_end(&mut rom.data)?;

        if rom.data.len() != rom.rom_size {
            bail!("Actual rom size is different from rom size data which is in ROM");
        }

        Ok(rom)
    }
}
