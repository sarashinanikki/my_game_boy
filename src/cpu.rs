use std::io;

use anyhow::{bail, Result};

use crate::{bus::Bus};
pub struct Cpu {
    A: u8,
    B: u8,
    C: u8,
    D: u8,
    E: u8,
    F: u8,
    H: u8,
    L: u8,
    SP: u16,
    PC: u16,
    pub bus: Bus,
    halt: bool,
    ime: bool,
    step_flag: bool,
    debug_flag: bool,
    break_points: Vec<u16>,
    jmp_flag: bool
}

#[derive(Default)]
pub struct Opcode {
    pub cb_prefix: bool,
    pub code: u8
}

impl Cpu {
    pub fn new(bus: Bus) -> Self {
        Self {
            A: Default::default(),
            B: Default::default(),
            C: Default::default(),
            D: Default::default(),
            E: Default::default(),
            F: Default::default(),
            H: Default::default(),
            L: Default::default(),
            SP: 0xFFFE,
            PC: 0x100,
            bus,
            halt: Default::default(),
            ime: Default::default(),
            step_flag: Default::default(),
            debug_flag: Default::default(),
            break_points: Default::default(),
            jmp_flag: false
        }
    }

    // メインループ
    pub fn run(&mut self) -> Result<()> {
        let max_cycle: usize = 70224;
        let mut current_cycle: usize = 0;
        // self.step_flag = true;
        // self.debug_flag = true;

        while current_cycle < max_cycle {
            // 現在のPCにブレークポイントが張られていないか確認
            self.check_break_points();
            // halt時は4サイクルずつPPUなどを進める
            let mut op_cycle = 4;

            if !self.halt {
                // 命令コードを取得
                let opcode: Opcode = self.read_inst()?;
    
                if self.debug_flag {
                    self.debug_output(&opcode);
                }
    
                // 命令コードを実行
                op_cycle = self.excute_op(&opcode)?;

                // ステップ実行が有効化されていた場合はステップ実行に
                if self.step_flag {
                    self.stepping(&opcode);
                }
                
                // PCをインクリメント
                if !self.jmp_flag {
                    self.increment_pc();
                }
                else {
                    self.jmp_flag = false;
                }
            }

            // 割り込みのフラグを立たせる
            self.update_interrupt();

            // PPUをサイクル分動かす
            op_cycle += self.check_interrupt();
            self.bus.ppu.tick(op_cycle);

            // Timerをサイクル分動かす
            self.bus.timer.tick(op_cycle);

            // 現在のサイクル数を更新
            current_cycle += op_cycle as usize;
        }

        Ok(())
    }

    fn update_interrupt(&mut self) {
        if self.bus.ppu.int_vblank {
            self.bus.ppu.int_vblank = false;
            self.bus.int_flag |= 1 << 0;
        }

        if self.bus.ppu.int_lcd_stat {
            self.bus.ppu.int_lcd_stat = false;
            self.bus.int_flag |= 1 << 1;
        }

        if self.bus.timer.int_timer_flag {
            self.bus.timer.int_timer_flag = false;
            self.bus.int_flag |= 1 << 2;
        }

        if self.bus.joypad.int_flag {
            self.bus.joypad.int_flag = false;
            self.bus.int_flag |= 1 << 4;
        }
    }

    fn check_interrupt(&mut self) -> u8 {
        let interrupt_flags = self.bus.int_flag & self.bus.ie_flag;

        if interrupt_flags != 0 {
            for bit in 0..5_u8 {
                if (interrupt_flags & (1 << bit)) == (1 << bit) {
                    let interrupt_idx = bit;
                    return self.handle_interrupt(interrupt_idx)
                }
            }
        }

        return 0;
    }

    fn handle_interrupt(&mut self, interrupt_idx: u8) -> u8 {
        if self.ime {
            self.bus.int_flag &= !(1 << interrupt_idx);
        }
        self.halt = false;

        let address = match interrupt_idx {
            0 => 0x40,
            1 => 0x48,
            2 => 0x50,
            3 => 0x58,
            4 | _ => {
                self.bus.timer.is_stop = false;
                0x60
            }
        };

        if self.ime {
            self.ime = false;
            self.int_call(address).unwrap();
        }

        return 12;
    }

    pub fn render(&mut self, frame: &mut [u8]) {
        self.bus.ppu.render(frame).unwrap();
    }

    // 現在のPCにブレークポイントが張られていた場合はステップ実行をON
    fn check_break_points(&mut self) {
        if self.break_points.contains(&self.PC) {
            self.step_flag = true;
        }
    }

    pub fn set_break_point(&mut self, bp: u16) {
        self.break_points.push(bp);
    }

    // ステップ実行
    fn stepping(&mut self, opcode: &Opcode) {
        // 現状を出力
        println!("Current Data:");
        self.debug_output(opcode);

        loop {
            let mut raw_command = String::new();
            io::stdin().read_line(&mut raw_command).expect("Failed to read");
            let command = raw_command.trim_end();

            match command {
                "n" => break,
                "debug" => {
                    self.debug_flag = !self.debug_flag;
                },
                "dump" => {
                    self.bus.ppu.dump();
                }
                "go" => {
                    self.step_flag = false;
                    break;
                },
                "set bp" => {
                    print!("set break point => ");
                    let mut raw_bp = String::new();
                    io::stdin().read_line(&mut raw_bp).expect("Failed to read");
                    let str_bp = raw_bp.trim_start_matches("0x").trim_end();
                    let bp = u16::from_str_radix(str_bp, 16).unwrap();
                    self.break_points.push(bp);
                },
                "rm bp" => {
                    print!("remove break point => ");
                    let mut raw_bp = String::new();
                    io::stdin().read_line(&mut raw_bp).expect("Failed to read");
                    let str_bp = raw_bp.trim_start_matches("0x").trim_end();
                    println!("str_bp = {}", str_bp);
                    let bp = u16::from_str_radix(str_bp, 16).unwrap();
                    if let Some(idx) = self.break_points.iter().position(|x| *x == bp) {
                        self.break_points.remove(idx);
                        self.step_flag = false;
                    }
                },
                _ => println!("unknown command")
            }
        }
    }

    // デバッグ情報を出力
    fn debug_output(&self, opcode: &Opcode) {
        println!(
            "PC: {:X}, opcode: {:X}, CB: {}\n 
            A: {:X}, B: {:X}, C: {:X}, D: {:X}, E: {:X}, F: {:X}, H: {:X}, L: {:X}, SP: {:X}, AF: {:X}, BC: {:X}, DE: {:X}, HL: {:X}",
            self.PC, opcode.code, opcode.cb_prefix, self.A, self.B, self.C, self.D, self.E, self.F, self.H, self.L, self.SP,
            self.get_af(), self.get_bc(), self.get_de(), self.get_hl()
        )
    }

    // 命令の読み込み
    fn read_inst(&mut self) -> Result<Opcode> {
        let opcode: Opcode = match self.bus.read(self.PC) {
            Ok(0xCB) => {
                self.increment_pc();
                match self.bus.read(self.PC) {
                    Ok(res) => Opcode { cb_prefix: true, code: res },
                    Err(_err) => bail!("fail! error occured reading a opcode. {}", _err)
                }
            },
            Ok(res) => Opcode { cb_prefix: false, code: res },
            Err(_err) => bail!("fail! error occured reading a opcode. {}", _err)
        };

        Ok(opcode)
    }

    pub fn read_next_8(&mut self) -> Result<u8> {
        self.increment_pc();
        let data: u8 = self.bus.read(self.PC)?;

        return Ok(data)
    }

    pub fn read_next_16(&mut self) -> Result<u16> {
        self.increment_pc();
        let lower: u16 = self.bus.read(self.PC)? as u16;
        self.increment_pc();
        let upper: u16 = self.bus.read(self.PC)? as u16;

        let ret: u16 = upper * 256 + lower;
        Ok(ret as u16)
    }

    fn increment_pc(&mut self) {
        self.PC = self.PC.wrapping_add(1);
    }

    fn increment_sp(&mut self) {
        self.SP = self.SP.wrapping_add(1);
    }

    fn decrement_sp(&mut self) {
        self.SP = self.SP.wrapping_sub(1);
    }

    fn decrement_hl(&mut self) {
        let hl = self.get_hl();
        self.set_hl(hl.wrapping_sub(1));
    }

    fn increment_hl(&mut self) {
        let hl = self.get_hl();
        self.set_hl(hl.wrapping_add(1));
    }

    fn increment_bc(&mut self) {
        let bc = self.get_bc();
        self.set_bc(bc.wrapping_add(1));
    }

    fn decrement_bc(&mut self) {
        let bc = self.get_bc();
        self.set_bc(bc.wrapping_sub(1));
    }

    fn increment_de(&mut self) {
        let de = self.get_de();
        self.set_de(de.wrapping_add(1));
    }

    fn decrement_de(&mut self) {
        let de = self.get_de();
        self.set_de(de.wrapping_sub(1));
    }

    fn get_af(&self) -> u16 {
        let a: u16 = self.A as u16;
        let f: u16 = self.F as u16;

        return a*256 + f;
    }

    fn get_bc(&self) -> u16 {
        let b: u16 = self.B as u16;
        let c: u16 = self.C as u16;

        return b*256 + c;
    }

    fn get_de(&self) -> u16 {
        let d: u16 = self.D as u16;
        let e: u16 = self.E as u16;

        return d*256 + e;
    }

    fn get_hl(&self) -> u16 {
        let h: u16 = self.H as u16;
        let l: u16 = self.L as u16;

        return h*256 + l;
    }

    fn set_af(&mut self, data: u16) {
        let a = data / 256;
        let f = data % 256;

        self.A = a as u8;
        self.F = f as u8;
    }

    fn set_bc(&mut self, data: u16) {
        let b = data / 256;
        let c = data % 256;

        self.B = b as u8;
        self.C = c as u8;
    }

    fn set_de(&mut self, data: u16) {
        let d = data / 256;
        let e = data % 256;

        self.D = d as u8;
        self.E = e as u8;
    }

    fn set_hl(&mut self, data: u16) {
        let h = data / 256;
        let l = data % 256;

        self.H = h as u8;
        self.L = l as u8;
    }

    // 命令の実行。返り値に命令のサイクルを返す
    fn excute_op(&mut self, opcode: &Opcode) -> Result<u8> {
        // 気合で分岐
        match opcode {
            Opcode { cb_prefix: false, code: res } => {
                match res {
                    0xD9 => self.reti(),
                    0xD8 => self.ret_c(),
                    0xD0 => self.ret_nc(),
                    0xC8 => self.ret_z(),
                    0xC0 => self.ret_nz(),
                    0xC9 => self.ret(),
                    0xC7 | 0xCF | 
                    0xD7 | 0xDF |
                    0xE7 | 0xEF |
                    0xF7 | 0xFF => self.rst(res),
                    0xDC => self.call_c(),
                    0xD4 => self.call_nc(),
                    0xCC => self.call_z(),
                    0xC4 => self.call_nz(),
                    0xCD => self.call(),
                    0x38 => self.jr_c(),
                    0x30 => self.jr_nc(),
                    0x28 => self.jr_z(),
                    0x20 => self.jr_nz(),
                    0x18 => self.jr(),
                    0xE9 => self.jp_hl(),
                    0xDA => self.jp_c(),
                    0xD2 => self.jp_nc(),
                    0xCA => self.jp_z(),
                    0xC2 => self.jp_nz(),
                    0xC3 => self.jp(),
                    0x1F => self.rra(),
                    0x0F => self.rrca(),
                    0x17 => self.rla(),
                    0x07 => self.rlca(),
                    0xFB => self.ei(),
                    0xF3 => self.di(),
                    0x10 => self.stop(),
                    0x76 => self.halt(),
                    0x00 => self.nop(),
                    0x37 => self.scf(),
                    0x3F => self.ccf(),
                    0x2F => self.cpl(),
                    0x27 => self.decimal_adjust_accumlator(),
                    0x3B => self.dec_3B(),
                    0x2B => self.dec_2B(),
                    0x1B => self.dec_1B(),
                    0x0B => self.dec_0B(),
                    0x33 => self.inc_33(),
                    0x23 => self.inc_23(),
                    0x13 => self.inc_13(),
                    0x03 => self.inc_03(),
                    0xE8 => self.add_E8(),
                    0x39 => self.add_39(),
                    0x29 => self.add_29(),
                    0x19 => self.add_19(),
                    0x09 => self.add_09(),
                    0x35 => self.dec_35(),
                    0x2D => self.dec_2D(),
                    0x25 => self.dec_25(),
                    0x1D => self.dec_1D(),
                    0x15 => self.dec_15(),
                    0x0D => self.dec_0D(),
                    0x05 => self.dec_05(),
                    0x3D => self.dec_3D(),
                    0x34 => self.inc_34(),
                    0x2C => self.inc_2C(),
                    0x24 => self.inc_24(),
                    0x1C => self.inc_1C(),
                    0x14 => self.inc_14(),
                    0x0C => self.inc_0C(),
                    0x04 => self.inc_04(),
                    0x3C => self.inc_3C(),
                    0xFE => self.cp_FE(),
                    0xBE => self.cp_BE(),
                    0xBD => self.cp_BD(),
                    0xBC => self.cp_BC(),
                    0xBB => self.cp_BB(),
                    0xBA => self.cp_BA(),
                    0xB9 => self.cp_B9(),
                    0xB8 => self.cp_B8(),
                    0xBF => self.cp_BF(),
                    0xEE => self.xor_EE(),
                    0xAE => self.xor_AE(),
                    0xAD => self.xor_AD(),
                    0xAC => self.xor_AC(),
                    0xAB => self.xor_AB(),
                    0xAA => self.xor_AA(),
                    0xA9 => self.xor_A9(),
                    0xA8 => self.xor_A8(),
                    0xAF => self.xor_AF(),
                    0xF6 => self.or_F6(),
                    0xB6 => self.or_B6(),
                    0xB5 => self.or_B5(),
                    0xB4 => self.or_B4(),
                    0xB3 => self.or_B3(),
                    0xB2 => self.or_B2(),
                    0xB1 => self.or_B1(),
                    0xB0 => self.or_B0(),
                    0xB7 => self.or_B7(),
                    0xE6 => self.and_E6(),
                    0xA6 => self.and_A6(),
                    0xA5 => self.and_A5(),
                    0xA4 => self.and_A4(),
                    0xA3 => self.and_A3(),
                    0xA2 => self.and_A2(),
                    0xA1 => self.and_A1(),
                    0xA0 => self.and_A0(),
                    0xA7 => self.and_A7(),
                    0x9E => self.sbc_9E(),
                    0x9D => self.sbc_9D(),
                    0x9C => self.sbc_9C(),
                    0x9B => self.sbc_9B(),
                    0x9A => self.sbc_9A(),
                    0x99 => self.sbc_99(),
                    0x98 => self.sbc_98(),
                    0x9F => self.sbc_9F(),
                    0xD6 => self.sub_D6(),
                    0x96 => self.sub_96(),
                    0x95 => self.sub_95(),
                    0x94 => self.sub_94(),
                    0x93 => self.sub_93(),
                    0x92 => self.sub_92(),
                    0x91 => self.sub_91(),
                    0x90 => self.sub_90(),
                    0x97 => self.sub_97(),
                    0xCE => self.adc_CE(),
                    0x8E => self.adc_8E(),
                    0x8D => self.adc_8D(),
                    0x8C => self.adc_8C(),
                    0x8B => self.adc_8B(),
                    0x8A => self.adc_8A(),
                    0x89 => self.adc_89(),
                    0x88 => self.adc_88(),
                    0x8F => self.adc_8F(),
                    0xC6 => self.add_C6(),
                    0x86 => self.add_86(),
                    0x85 => self.add_85(),
                    0x84 => self.add_84(),
                    0x83 => self.add_83(),
                    0x82 => self.add_82(),
                    0x81 => self.add_81(),
                    0x80 => self.add_80(),
                    0x87 => self.add_87(),
                    0xE1 => self.pop_E1(),
                    0xD1 => self.pop_D1(),
                    0xC1 => self.pop_C1(),
                    0xF1 => self.pop_F1(),
                    0xE5 => self.push_E5(),
                    0xD5 => self.push_D5(),
                    0xC5 => self.push_C5(),
                    0xF5 => self.push_F5(),
                    0x08 => self.ld_08(),
                    0xF8 => self.ld_F8(),
                    0xF9 => self.ld_F9(),
                    0x31 => self.ld_31(),
                    0x21 => self.ld_21(),
                    0x11 => self.ld_11(),
                    0x01 => self.ld_01(),
                    0xF0 => self.ld_F0(),
                    0xE0 => self.ld_E0(),
                    0x22 => self.ld_22(),
                    0x2A => self.ld_2A(),
                    0x32 => self.ld_32(),
                    0x3A => self.ld_3A(),
                    0xE2 => self.ld_E2(),
                    0xF2 => self.ld_F2(),
                    0xEA => self.ld_EA(),
                    0x77 => self.ld_77(),
                    0x12 => self.ld_12(),
                    0x02 => self.ld_02(),
                    0x67 => self.ld_67(),
                    0x5F => self.ld_5F(),
                    0x57 => self.ld_57(),
                    0x4F => self.ld_4F(),
                    0x47 => self.ld_47(),
                    0x3E => self.ld_3E(),
                    0xFA => self.ld_FA(),
                    0x1A => self.ld_1A(),
                    0x0A => self.ld_0A(),
                    0x36 => self.ld_36(),
                    0x75 => self.ld_75(),
                    0x74 => self.ld_74(),
                    0x73 => self.ld_73(),
                    0x72 => self.ld_72(),
                    0x71 => self.ld_71(),
                    0x70 => self.ld_70(),
                    0x6F => self.ld_6F(),
                    0x6E => self.ld_6E(),
                    0x6D => self.ld_6D(),
                    0x6C => self.ld_6C(),
                    0x6B => self.ld_6B(),
                    0x6A => self.ld_6A(),
                    0x69 => self.ld_69(),
                    0x68 => self.ld_68(),
                    0x66 => self.ld_66(),
                    0x65 => self.ld_65(),
                    0x64 => self.ld_64(),
                    0x63 => self.ld_63(),
                    0x62 => self.ld_62(),
                    0x61 => self.ld_61(),
                    0x60 => self.ld_60(),
                    0x5E => self.ld_5E(),
                    0x5D => self.ld_5D(),
                    0x5C => self.ld_5C(),
                    0x5B => self.ld_5B(),
                    0x5A => self.ld_5A(),
                    0x59 => self.ld_59(),
                    0x58 => self.ld_58(),
                    0x56 => self.ld_56(),
                    0x55 => self.ld_55(),
                    0x54 => self.ld_54(),
                    0x53 => self.ld_53(),
                    0x52 => self.ld_52(),
                    0x51 => self.ld_51(),
                    0x50 => self.ld_50(),
                    0x4E => self.ld_4E(),
                    0x4D => self.ld_4D(),
                    0x4C => self.ld_4C(),
                    0x4B => self.ld_4B(),
                    0x4A => self.ld_4A(),
                    0x49 => self.ld_49(),
                    0x48 => self.ld_48(),
                    0x46 => self.ld_46(),
                    0x45 => self.ld_45(),
                    0x44 => self.ld_44(),
                    0x43 => self.ld_43(),
                    0x42 => self.ld_42(),
                    0x41 => self.ld_41(),
                    0x40 => self.ld_40(),
                    0x06 => self.ld_06(),
                    0x0E => self.ld_0E(),
                    0x16 => self.ld_16(),
                    0x1E => self.ld_1E(),
                    0x26 => self.ld_26(),
                    0x2E => self.ld_2E(),
                    0x7F => self.ld_7F(),
                    0x78 => self.ld_78(),
                    0x79 => self.ld_79(),
                    0x7A => self.ld_7A(),
                    0x7B => self.ld_7B(),
                    0x7C => self.ld_7C(),
                    0x7D => self.ld_7D(),
                    0x7E => self.ld_7E(),
                    _ => bail!("unknown opcode!")
                }
            },
            Opcode { cb_prefix: true, code: res } => {
                match res {
                    06 => Ok(0),
                    0x00..=0x07 => self.rlc_CB(res),
                    0x08..=0x0F => self.rrc_CB(res),
                    0x10..=0x17 => self.rl_CB(res),
                    0x18..=0x1F => self.rr_CB(res),
                    0x20..=0x27 => self.sla_CB(res),
                    0x28..=0x2F => self.sra_CB(res),
                    0x30..=0x37 => self.swap_CB(res),
                    0x38..=0x3F => self.srl_CB(res),
                    0x40..=0x7F => self.bit_CB(res),
                    0x80..=0xBF => self.res_CB(res),
                    0xC0..=0xFF => self.set_CB(res)
                }
            }
        }
    }

    // (h, c)を返す。8bitの足し算時に使う
    fn is_carry_positive(&mut self, left: u16, right: u16) -> (bool, bool) {
        let h: bool = ((left & 0x0F) + (right & 0x0F) & 0x10) == 0x10;
        let c: bool = ((left & 0xFF) as u16 + (right & 0xFF) as u16 & 0x100) == 0x100;

        return (h, c)
    }

    // (h, c)を返す。8bitの引き算時に使う
    fn is_carry_negative(&mut self, left: u8, right: u16) -> (bool, bool) {
        let h: bool = (left as u16 & 0x0F) < (right & 0x0F);
        let c: bool = (left as u16 & 0xFF) < (right & 0xFF);

        return (h, c);
    }

    // (h, c)を返す。16bitの足し算時に使う
    fn is_carry_positive_16(&mut self, left: u16, right: u16) -> (bool, bool) {
        let h: bool = ((left & 0xFFF) + (right & 0xFFF) & 0x1000) == 0x1000;
        let c: bool = ((left & 0xFFFF) as u32 + (right & 0xFFFF) as u32 & 0x10000) == 0x10000;

        return (h, c)
    }

    // (h, c)を返す。16bitの引き算時に使う
    fn is_carry_negative_16(&mut self, left: u16, right: u16) -> (bool, bool) {
        let h: bool = (left & 0xFFF) < (right & 0xFFF);
        let c: bool = (left & 0xFFFF) < (right & 0xFFFF);

        return (h, c);
    }

    fn set_flag(&mut self, z: bool, n: bool, h: bool, c: bool) {
        let mut f: u8 = 0;
        if z {
            f |= 1 << 7;
        }

        if n {
            f |= 1 << 6;
        }

        if h {
            f |= 1 << 5
        }

        if c {
            f |= 1 << 4;
        }

        self.F = f;
    }

    fn get_carry_flag(&self) -> bool {
        return (self.F & (1 << 4)) == 1 << 4;
    }

    fn get_half_carry_flag(&self) -> bool {
        return (self.F & (1 << 5)) == 1 << 5;
    }

    fn get_zero_flag(&self) -> bool {
        return (self.F & (1 << 7)) == 1 << 7;
    }

    fn get_n_flag(&self) -> bool {
        return (self.F & (1 << 6)) == 1 << 6;
    }

    fn swap_8bit(&self, target: u8) -> u8 {
        let upper: u8 = target & 0xF0;
        let lower: u8 = target & 0x0F;

        let new_value: u8 = (lower << 4) + (upper >> 4);
        return new_value;
    }

    fn swap_16bit(&self, target: u16) -> u16 {
        let upper: u16 = target & 0xFF00;
        let lower: u16 = target & 0x00FF;

        let new_value: u16 = (lower << 8) + (upper >> 8);
        return new_value;
    }

    // (レジスタの値, u8にキャストするかどうか)を返す
    fn opcode_to_read_registers(&self, opcode: &u8) -> (u16, bool) {
        let lower = opcode & 0x0F;
        let ret: (u16, bool) = match lower {
            0x07 | 0x0F => (self.A as u16, true),
            0x00 | 0x08 => (self.B as u16, true),
            0x01 | 0x09 => (self.C as u16, true),
            0x02 | 0x0A => (self.D as u16, true),
            0x03 | 0x0B => (self.E as u16, true),
            0x04 | 0x0C => (self.H as u16, true),
            0x05 | 0x0D => (self.L as u16, true),
            0x06 | 0x0E => (self.get_hl(), false),
            _ => (0, false)
        };
        return ret;
    }

    fn opcode_to_write_registers(&mut self, opcode: &u8, data: u8) {
        let lower = opcode & 0x0F;
        match lower {
            0x07 | 0x0F => self.A = data,
            0x00 | 0x08 => self.B = data,
            0x01 | 0x09 => self.C = data,
            0x02 | 0x0A => self.D = data,
            0x03 | 0x0B => self.E = data,
            0x04 | 0x0C => self.H = data,
            0x05 | 0x0D => self.L = data,
            _ => {}
        };
    }

    fn get_bit_target(&self, opcode: &u8, first: u8) -> u8 {
        let upper_opcode = (opcode & 0xF0) >> 4;
        let lower_opcode = opcode & 0x0F;

        let mut target = (upper_opcode - first) * 2;

        if lower_opcode > 0x07 {
            target += 1;
        }

        return target;
    }

    // region: inst
    #[allow(dead_code)]
    fn reti(&mut self) -> Result<u8> {
        let stack_address = self.SP;
        let address = self.bus.read_16(stack_address)?;
        self.PC = address;
        self.jmp_flag = true;

        // 2byteのデータをpopするので2回インクリメント
        self.increment_sp();
        self.increment_sp();

        // 割り込みを有効化
        self.ime = true;
        Ok(16)
    }

    #[allow(dead_code)]
    fn ret_c(&mut self) -> Result<u8> {
        let mut cycle = 8;
        let c = self.get_carry_flag();
        
        if c {
            let stack_address = self.SP;
            let address = self.bus.read_16(stack_address)?;
            self.PC = address;
            self.jmp_flag = true;

            // 2byteのデータをpopするので2回インクリメント
            self.increment_sp();
            self.increment_sp();
            cycle = 20;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn ret_nc(&mut self) -> Result<u8> {
        let mut cycle = 8;
        let c = self.get_carry_flag();
        
        if !c {
            let stack_address = self.SP;
            let address = self.bus.read_16(stack_address)?;
            self.PC = address;
            self.jmp_flag = true;

            // 2byteのデータをpopするので2回インクリメント
            self.increment_sp();
            self.increment_sp();
            cycle = 20;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn ret_z(&mut self) -> Result<u8> {
        let mut cycle = 8;
        let z = self.get_zero_flag();
        
        if z {
            let stack_address = self.SP;
            let address = self.bus.read_16(stack_address)?;
            self.PC = address;
            self.jmp_flag = true;

            // 2byteのデータをpopするので2回インクリメント
            self.increment_sp();
            self.increment_sp();
            cycle = 20;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn ret_nz(&mut self) -> Result<u8> {
        let mut cycle = 8;
        let z = self.get_zero_flag();
        
        if !z {
            let stack_address = self.SP;
            let address = self.bus.read_16(stack_address)?;
            self.PC = address;
            self.jmp_flag = true;

            // 2byteのデータをpopするので2回インクリメント
            self.increment_sp();
            self.increment_sp();
            cycle = 20;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn ret(&mut self) -> Result<u8> {
        let stack_address = self.SP;
        let address = self.bus.read_16(stack_address)?;
        self.PC = address;
        self.jmp_flag = true;

        // 2byteのデータをpopするので2回インクリメント
        self.increment_sp();
        self.increment_sp();

        Ok(16)
    }

    #[allow(dead_code)]
    fn rst(&mut self, opcode: &u8) -> Result<u8> {
        let address = match opcode {
            0xC7 => 0,
            0xCF => 8,
            0xD7 => 10,
            0xDF => 18,
            0xE7 => 20,
            0xEF => 28,
            0xF7 => 30,
            0xFF => 38,
            _ => bail!("invalid rst opcode!")
        };
        
        self.base_call(address)?;

        Ok(16)
    }

    #[allow(dead_code)]
    fn call_c(&mut self) -> Result<u8> {
        let address: u16 = self.read_next_16()?;
        let mut cycle = 6;
        let c = self.get_carry_flag();
        
        if c {
            self.base_call(address)?;
            cycle = 12;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn call_nc(&mut self) -> Result<u8> {
        let address: u16 = self.read_next_16()?;
        let mut cycle = 6;
        let c = self.get_carry_flag();
        
        if !c {
            self.base_call(address)?;
            cycle = 12;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn call_z(&mut self) -> Result<u8> {
        let address: u16 = self.read_next_16()?;
        let mut cycle = 6;
        let z = self.get_zero_flag();
        
        if z {
            self.base_call(address)?;
            cycle = 12;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn call_nz(&mut self) -> Result<u8> {
        let address: u16 = self.read_next_16()?;
        let mut cycle = 6;
        let z = self.get_zero_flag();
        
        if !z {
            self.base_call(address)?;
            cycle = 12;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn call(&mut self) -> Result<u8> {
        let address = self.read_next_16()?;
        self.base_call(address)?;
        Ok(12)
    }

    fn base_call(&mut self, address: u16) -> Result<()> {
        // 2byteのデータを積むので2回デクリメント
        self.decrement_sp();
        self.decrement_sp();

        let stack_address = self.SP;
        self.bus.write_16(stack_address, self.PC.wrapping_add(1))?;
        self.PC = address;
        self.jmp_flag = true;

        Ok(())
    }

    fn int_call(&mut self, address: u16) -> Result<()> {
        // 2byteのデータを積むので2回デクリメント
        self.decrement_sp();
        self.decrement_sp();

        let stack_address = self.SP;
        self.bus.write_16(stack_address, self.PC)?;
        self.PC = address;
        self.jmp_flag = true;

        Ok(())
    }

    #[allow(dead_code)]
    fn jr_c(&mut self) -> Result<u8> {
        let address = self.read_next_8()?;
        let mut cycle = 8;
        let c = self.get_carry_flag();
        let offset: i8 = address as i8;
        let pc = self.PC as isize + offset as isize + 1;
        
        if c {
            self.PC = pc as u16;
            self.jmp_flag = true;
            cycle = 12;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn jr_nc(&mut self) -> Result<u8> {
        let address = self.read_next_8()?;
        let mut cycle = 8;
        let c = self.get_carry_flag();
        let offset: i8 = address as i8;
        let pc = self.PC as isize + offset as isize + 1;
        
        if !c {
            self.PC = pc as u16;
            self.jmp_flag = true;
            cycle = 12;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn jr_z(&mut self) -> Result<u8> {
        let address = self.read_next_8()?;
        let mut cycle = 8;
        let z = self.get_zero_flag();
        let offset: i8 = address as i8;
        let pc = self.PC as isize + offset as isize + 1;
        
        if z {
            self.PC = pc as u16;
            self.jmp_flag = true;
            cycle = 12;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn jr_nz(&mut self) -> Result<u8> {
        let address = self.read_next_8()?;
        let mut cycle = 8;
        let z = self.get_zero_flag();
        let offset: i8 = address as i8;
        let pc = self.PC as isize + offset as isize + 1;
        
        if !z {
            self.PC = pc as u16;
            self.jmp_flag = true;
            cycle = 12;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn jr(&mut self) -> Result<u8> {
        let address = self.read_next_8()?;
        let offset: i8 = address as i8;
        let pc = self.PC as isize + offset as isize + 1;
        self.PC = pc as u16;
        self.jmp_flag = true;

        Ok(12)
    }

    #[allow(dead_code)]
    fn jp_hl(&mut self) -> Result<u8> {
        let address: u16 = self.get_hl();
        self.PC = address;
        self.jmp_flag = true;

        Ok(4)
    }

    #[allow(dead_code)]
    fn jp_c(&mut self) -> Result<u8> {
        let address: u16 = self.read_next_16()?;
        let mut cycle = 12;
        let c = self.get_carry_flag();
        
        if c {
            self.PC = address;
            self.jmp_flag = true;
            cycle = 16;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn jp_nc(&mut self) -> Result<u8> {
        let address: u16 = self.read_next_16()?;
        let mut cycle = 12;
        let c = self.get_carry_flag();
        
        if !c {
            self.PC = address;
            self.jmp_flag = true;
            cycle = 16;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn jp_z(&mut self) -> Result<u8> {
        let address: u16 = self.read_next_16()?;
        let mut cycle = 12;
        let z = self.get_zero_flag();
        
        if z {
            self.PC = address;
            self.jmp_flag = true;
            cycle = 16;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn jp_nz(&mut self) -> Result<u8> {
        let address: u16 = self.read_next_16()?;
        let mut cycle = 12;
        let z = self.get_zero_flag();
        
        if !z {
            self.PC = address;
            self.jmp_flag = true;
            cycle = 16;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn jp(&mut self) -> Result<u8> {
        let address: u16 = self.read_next_16()?;
        self.PC = address;
        self.jmp_flag = true;

        Ok(16)
    }

    #[allow(dead_code)]
    fn res_CB(&mut self, opcode: &u8) -> Result<u8> {
        let (register_val, is_cast) = self.opcode_to_read_registers(opcode);
        let mut cycle: u8 = 8;
        let target_bit: u8 = self.get_bit_target(opcode, 8);

        if is_cast {
            let target = register_val as u8;
            let data = target & !(1 << target_bit);
            self.opcode_to_write_registers(opcode, data);
        }
        else {
            let address = register_val;
            let target = self.bus.read(address)?;
            let data = target & !(1 << target_bit);
            self.bus.write(address, data)?;
            
            cycle = 16;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn set_CB(&mut self, opcode: &u8) -> Result<u8> {
        let (register_val, is_cast) = self.opcode_to_read_registers(opcode);
        let mut cycle: u8 = 8;
        let target_bit: u8 = self.get_bit_target(opcode, 0x0C);

        if is_cast {
            let target = register_val as u8;
            let data = target | (1 << target_bit);
            self.opcode_to_write_registers(opcode, data);
        }
        else {
            let address = register_val;
            let target = self.bus.read(address)?;
            let data = target | (1 << target_bit);
            self.bus.write(address, data)?;
            
            cycle = 16;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn bit_CB(&mut self, opcode: &u8) -> Result<u8> {
        let (register_val, is_cast) = self.opcode_to_read_registers(opcode);
        let mut cycle: u8 = 8;
        let target_bit: u8 = self.get_bit_target(opcode, 4);
        let c = self.get_carry_flag();

        if is_cast {
            let target = register_val as u8;
            let z = !((target & (1 << target_bit)) == 1 << target_bit);
            
            self.set_flag(z, false, true, c);
        }
        else {
            let address = register_val;
            let target = self.bus.read(address)?;
            let z = !((target & (1 << target_bit)) == 1 << target_bit);
            
            self.set_flag(z, false, true, c);
            cycle = 12;
        }

        Ok(cycle)
    }

    // 論理シフト
    #[allow(dead_code)]
    fn srl_CB(&mut self, opcode: &u8) -> Result<u8> {
        let (register_val, is_cast) = self.opcode_to_read_registers(opcode);
        let mut cycle: u8 = 8;

        if is_cast {
            let target = register_val as u8;
            let c = (target & (1 << 0)) == 1 << 0;
            let val = target >> 1;
            
            self.opcode_to_write_registers(opcode, val);
            let z = val == 0;
            self.set_flag(z, false, false, c);
        }
        else {
            let address = register_val;
            let target = self.bus.read(address)?;
            let c = (target & (1 << 0)) == 1 << 0;
            let val = target >> 1;

            self.bus.write(address, val)?;
            let z = val == 0;
            self.set_flag(z, false, false, c);
            cycle = 16;
        }

        Ok(cycle)
    }

    // 算術シフト
    #[allow(dead_code)]
    fn sra_CB(&mut self, opcode: &u8) -> Result<u8> {
        let (register_val, is_cast) = self.opcode_to_read_registers(opcode);
        let mut cycle: u8 = 8;

        if is_cast {
            let target = register_val as u8;
            let c = (target & (1 << 0)) == 1 << 0;
            let msb = (target & (1 << 7)) == 1 << 7;
            let mut val = target >> 1;
            
            if msb {
                val |= 1 << 7;
            }
            
            self.opcode_to_write_registers(opcode, val);
            let z = val == 0;
            self.set_flag(z, false, false, c);
        }
        else {
            let address = register_val;
            let target = self.bus.read(address)?;
            let c = (target & (1 << 0)) == 1 << 0;
            let msb = (target & (1 << 7)) == 1 << 7;
            let mut val = target >> 1;

            if msb {
                val |= 1 << 7;
            }
            
            self.bus.write(address, val)?;
            let z = val == 0;
            self.set_flag(z, false, false, c);
            cycle = 16;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn sla_CB(&mut self, opcode: &u8) -> Result<u8> {
        let (register_val, is_cast) = self.opcode_to_read_registers(opcode);
        let mut cycle: u8 = 8;

        if is_cast {
            let target = register_val as u8;
            let c = (target & (1 << 7)) == 1 << 7;
            let val = target << 1;
            
            self.opcode_to_write_registers(opcode, val);
            let z = val == 0;
            self.set_flag(z, false, false, c);
        }
        else {
            let address = register_val;
            let target = self.bus.read(address)?;
            let c = (target & (1 << 7)) == 1 << 7;
            let val = target << 1;
            
            self.bus.write(address, val)?;
            let z = val == 0;
            self.set_flag(z, false, false, c);
            cycle = 16;
        }

        Ok(cycle)
    }
    
    #[allow(dead_code)]
    fn rr_CB(&mut self, opcode: &u8) -> Result<u8> {
        let (register_val, is_cast) = self.opcode_to_read_registers(opcode);
        let mut cycle: u8 = 8;

        if is_cast {
            let target = register_val as u8;
            let c = self.get_carry_flag();
            let c_new_flag = (target & (1 << 0)) == 1 << 0;
    
            let mut old_val = target;
            if c != c_new_flag {
                old_val = target ^ (1 << 0);
            }
            let val = old_val.rotate_right(1);
            
            self.opcode_to_write_registers(opcode, val);
            let z = val == 0;
            self.set_flag(z, false, false, c_new_flag);
        }
        else {
            let address = register_val;
            let target = self.bus.read(address)?;
            let c = self.get_carry_flag();
            let c_new_flag = (target & (1 << 0)) == 1 << 0;
    
            let mut old_val = target;
            if c != c_new_flag {
                old_val = target ^ (1 << 0);
            }
            let val = old_val.rotate_right(1);
            
            self.bus.write(address, val)?;
            let z = val == 0;
            self.set_flag(z, false, false, c_new_flag);
            cycle = 16;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn rrc_CB(&mut self, opcode: &u8) -> Result<u8> {
        let (register_val, is_cast) = self.opcode_to_read_registers(opcode);
        let mut cycle: u8 = 8;

        if is_cast {
            let target = register_val as u8;
            let c = (target & (1 << 0)) == 1 << 0;
            let val = target.rotate_right(1);
            
            self.opcode_to_write_registers(opcode, val);
            let z = val == 0;
            self.set_flag(z, false, false, c);
        }
        else {
            let address = register_val;
            let target = self.bus.read(address)?;
            let c = (target & (1 << 0)) == 1 << 0;
            let val = target.rotate_right(1);
            
            self.bus.write(address, val)?;
            let z = val == 0;
            self.set_flag(z, false, false, c);
            cycle = 16;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn rl_CB(&mut self, opcode: &u8) -> Result<u8> {
        let (register_val, is_cast) = self.opcode_to_read_registers(opcode);
        let mut cycle: u8 = 8;

        if is_cast {
            let target = register_val as u8;
            let c = self.get_carry_flag();
            let c_new_flag = (target & (1 << 7)) == 1 << 7;
    
            let mut old_val = target;
            if c != c_new_flag {
                old_val = target ^ (1 << 7);
            }
            let val = old_val.rotate_left(1);
            
            self.opcode_to_write_registers(opcode, val);
            let z = val == 0;
            self.set_flag(z, false, false, c_new_flag);
        }
        else {
            let address = register_val;
            let target = self.bus.read(address)?;
            let c = self.get_carry_flag();
            let c_new_flag = (target & (1 << 7)) == 1 << 7;
    
            let mut old_val = target;
            if c != c_new_flag {
                old_val = target ^ (1 << 7);
            }
            let val = old_val.rotate_left(1);
            
            self.bus.write(address, val)?;
            let z = val == 0;
            self.set_flag(z, false, false, c_new_flag);
            cycle = 16;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn rlc_CB(&mut self, opcode: &u8) -> Result<u8> {
        let (register_val, is_cast) = self.opcode_to_read_registers(opcode);
        let mut cycle: u8 = 8;

        if is_cast {
            let target = register_val as u8;
            let c = (target & (1 << 7)) == 1 << 7;
            let val = target.rotate_left(1);
            
            self.opcode_to_write_registers(opcode, val);
            let z = val == 0;
            self.set_flag(z, false, false, c);
        }
        else {
            let address = register_val;
            let target = self.bus.read(address)?;
            let c = (target & (1 << 7)) == 1 << 7;
            let val = target.rotate_left(1);
            
            self.bus.write(address, val)?;
            let z = val == 0;
            self.set_flag(z, false, false, c);
            cycle = 16;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn rra(&mut self) -> Result<u8> {
        let c = self.get_carry_flag();
        let c_new_flag = (self.A & (1 << 0)) == 1 << 0;

        let mut old_val = self.A;
        if c != c_new_flag {
            old_val = self.A ^ (1 << 0);
        }

        let val = old_val.rotate_right(1);
        self.A = val;

        let z = val == 0;
        self.set_flag(z, false, false, c_new_flag);

        Ok(4)
    }

    #[allow(dead_code)]
    fn rrca(&mut self) -> Result<u8> {
        let c = (self.A & (1 << 0)) == 1 << 0;
        let val = self.A.rotate_right(1);
        self.A = val;
        let z = val == 0;
        self.set_flag(z, false, false, c);

        Ok(4)
    }

    #[allow(dead_code)]
    fn rla(&mut self) -> Result<u8> {
        let c = self.get_carry_flag();
        let c_new_flag = (self.A & (1 << 7)) == 1 << 7;

        let mut old_val = self.A;
        if c != c_new_flag {
            old_val = self.A ^ (1 << 7);
        }

        let val = old_val.rotate_left(1);
        self.A = val;

        let z = val == 0;
        self.set_flag(z, false, false, c_new_flag);

        Ok(4)
    }

    #[allow(dead_code)]
    fn rlca(&mut self) -> Result<u8> {
        let c = (self.A & (1 << 7)) == 1 << 7;
        let val = self.A.rotate_left(1);
        self.A = val;
        let z = val == 0;
        self.set_flag(z, false, false, c);

        Ok(4)
    }

    #[allow(dead_code)]
    fn ei(&mut self) -> Result<u8> {
        self.ime = true;
        Ok(4)
    }
    
    #[allow(dead_code)]
    fn di(&mut self) -> Result<u8> {
        self.ime = false;
        Ok(4)
    }

    #[allow(dead_code)]
    fn stop(&mut self) -> Result<u8> {
        self.halt = true;
        self.bus.timer.is_stop = true;
        // TODO: LCDディスプレイも止める実装をする
        Ok(4)
    }

    #[allow(dead_code)]
    fn halt(&mut self) -> Result<u8> {
        self.halt = true;
        Ok(4)
    }

    #[allow(dead_code)]
    fn nop(&mut self) -> Result<u8> {
        Ok(4)
    }

    #[allow(dead_code)]
    fn scf(&mut self) -> Result<u8> {
        let z = self.get_zero_flag();
        self.set_flag(z, false, false, true);

        Ok(4)
    }

    #[allow(dead_code)]
    fn ccf(&mut self) -> Result<u8> {
        let z = self.get_zero_flag();
        let c = self.get_carry_flag() ^ true;
        self.set_flag(z, false, false, c);

        Ok(4)
    }

    #[allow(dead_code)]
    fn cpl(&mut self) -> Result<u8> {
        let val = self.A;
        let fliped_val = val ^ 0xFF;
        self.A = fliped_val;

        let z = self.get_zero_flag();
        let c = self.get_carry_flag();
        self.set_flag(z, true, true, c);

        Ok(4)
    }

    #[allow(dead_code)]
    fn decimal_adjust_accumlator(&mut self) -> Result<u8> {
        let mut val = self.A;
        let n_flag = self.get_n_flag();
        let c_flag = self.get_carry_flag();
        let h_flag = self.get_half_carry_flag();

        let mut c_new_flag = c_flag;

        if !n_flag {
            if c_flag || self.A > 0x99 {
                val = val.wrapping_add(0x60);
                c_new_flag = true;
            }
            if h_flag || (self.A & 0x0F) > 0x09 {
                val = val.wrapping_add(0x06);
            }
        }
        else {
            if c_flag {
                val = val.wrapping_sub(0x60);
            }
            if h_flag {
                val = val.wrapping_sub(0x06);
            }
        }

        self.A = val;
        let z_new_flag = self.A == 0;
        self.set_flag(z_new_flag, n_flag, false, c_new_flag);

        Ok(4)
    }

    #[allow(dead_code)]
    fn swap_CB(&mut self, opcode: &u8) -> Result<u8> {
        let (register_val, is_cast) = self.opcode_to_read_registers(opcode);
        let mut cycle: u8 = 8;

        if is_cast {
            let target = register_val as u8;
            let swaped_val = self.swap_8bit(target);
            self.A = swaped_val;
            let z: bool = swaped_val == 0;
            let (n, h, c) = (false, false, false);
            self.set_flag(z, n, h, c);
        }
        else {
            let target = register_val;
            let swaped_val = self.swap_16bit(target);
            self.set_hl(swaped_val);
            let z: bool = swaped_val == 0;
            let (n, h, c) = (false, false, false);
            self.set_flag(z, n, h, c);
            cycle = 16;
        }

        Ok(cycle)
    }

    #[allow(dead_code)]
    fn dec_3B(&mut self) -> Result<u8> {
        self.decrement_sp();
        Ok(8)
    }

    #[allow(dead_code)]
    fn dec_2B(&mut self) -> Result<u8> {
        self.decrement_hl();
        Ok(8)
    }

    #[allow(dead_code)]
    fn dec_1B(&mut self) -> Result<u8> {
        self.decrement_de();
        Ok(8)
    }

    #[allow(dead_code)]
    fn dec_0B(&mut self) -> Result<u8> {
        self.decrement_bc();
        Ok(8)
    }

    #[allow(dead_code)]
    fn inc_33(&mut self) -> Result<u8> {
        self.increment_sp();
        Ok(8)
    }

    #[allow(dead_code)]
    fn inc_23(&mut self) -> Result<u8> {
        self.increment_hl();
        Ok(8)
    }

    #[allow(dead_code)]
    fn inc_13(&mut self) -> Result<u8> {
        self.increment_de();
        Ok(8)
    }

    #[allow(dead_code)]
    fn inc_03(&mut self) -> Result<u8> {
        self.increment_bc();
        Ok(8)
    }

    #[allow(dead_code)]
    fn add_E8(&mut self) -> Result<u8> {
        let left = self.SP;
        let right = self.read_next_8()? as i8;
        let val = left as isize + right as isize;

        self.SP = val as u16;

        let z: bool = false;
        let n: bool = false;
        let (h, c) = self.is_carry_positive(left, right as u16);

        self.set_flag(z, n, h, c);
        Ok(16)
    }

    #[allow(dead_code)]
    fn add_39(&mut self) -> Result<u8> {
        let left = self.get_hl();
        let right = self.SP;
        let val = left.wrapping_add(right);

        self.set_hl(val);

        let z: bool = self.get_zero_flag();
        let n: bool = false;
        let (h, c) = self.is_carry_positive_16(left, right);

        self.set_flag(z, n, h, c);
        Ok(8)
    }

    #[allow(dead_code)]
    fn add_29(&mut self) -> Result<u8> {
        let left = self.get_hl();
        let right = self.get_hl();
        let val = left.wrapping_add(right);

        self.set_hl(val);

        let z: bool = self.get_zero_flag();
        let n: bool = false;
        let (h, c) = self.is_carry_positive_16(left, right);

        self.set_flag(z, n, h, c);
        Ok(8)
    }

    #[allow(dead_code)]
    fn add_19(&mut self) -> Result<u8> {
        let left = self.get_hl();
        let right = self.get_de();
        let val = left.wrapping_add(right);

        self.set_hl(val);

        let z: bool = self.get_zero_flag();
        let n: bool = false;
        let (h, c) = self.is_carry_positive_16(left, right);

        self.set_flag(z, n, h, c);
        Ok(8)
    }

    #[allow(dead_code)]
    fn add_09(&mut self) -> Result<u8> {
        let left = self.get_hl();
        let right = self.get_bc();
        let val = left.wrapping_add(right);

        self.set_hl(val);

        let z: bool = self.get_zero_flag();
        let n: bool = false;
        let (h, c) = self.is_carry_positive_16(left, right);

        self.set_flag(z, n, h, c);
        Ok(8)
    }

    #[allow(dead_code)]
    fn dec_35(&mut self) -> Result<u8> {
        let address = self.get_hl();
        let left = self.bus.read(address)?;
        let right = 1;
        let val = left.wrapping_sub(right);

        self.bus.write(address, val)?;

        let z: bool = val == 0;
        let n: bool = true;
        // Cは影響を受けない
        let (h, _) = self.is_carry_negative(left, right as u16);
        let c = self.get_carry_flag();

        self.set_flag(z, n, h, c);
        Ok(12)
    }
    
    #[allow(dead_code)]
    fn dec_2D(&mut self) -> Result<u8> {
        let left = self.L;
        let right = 1;
        let val = left.wrapping_sub(right);

        self.L = val;

        let z: bool = val == 0;
        let n: bool = true;
        // Cは影響を受けない
        let (h, _) = self.is_carry_negative(left, right as u16);
        let c = self.get_carry_flag();

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn dec_25(&mut self) -> Result<u8> {
        let left = self.H;
        let right = 1;
        let val = left.wrapping_sub(right);

        self.H = val;

        let z: bool = val == 0;
        let n: bool = true;
        // Cは影響を受けない
        let (h, _) = self.is_carry_negative(left, right as u16);
        let c = self.get_carry_flag();

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn dec_1D(&mut self) -> Result<u8> {
        let left = self.E;
        let right = 1;
        let val = left.wrapping_sub(right);

        self.E = val;

        let z: bool = val == 0;
        let n: bool = true;
        // Cは影響を受けない
        let (h, _) = self.is_carry_negative(left, right as u16);
        let c = self.get_carry_flag();

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn dec_15(&mut self) -> Result<u8> {
        let left = self.D;
        let right = 1;
        let val = left.wrapping_sub(right);

        self.D = val;

        let z: bool = val == 0;
        let n: bool = true;
        // Cは影響を受けない
        let (h, _) = self.is_carry_negative(left, right as u16);
        let c = self.get_carry_flag();

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn dec_0D(&mut self) -> Result<u8> {
        let left = self.C;
        let right = 1;
        let val = left.wrapping_sub(right);

        self.C = val;

        let z: bool = val == 0;
        let n: bool = true;
        // Cは影響を受けない
        let (h, _) = self.is_carry_negative(left, right as u16);
        let c = self.get_carry_flag();

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn dec_05(&mut self) -> Result<u8> {
        let left = self.B;
        let right = 1;
        let val = left.wrapping_sub(right);

        self.B = val;

        let z: bool = val == 0;
        let n: bool = true;
        // Cは影響を受けない
        let (h, _) = self.is_carry_negative(left, right as u16);
        let c = self.get_carry_flag();

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn dec_3D(&mut self) -> Result<u8> {
        let left = self.A;
        let right = 1;
        let val = left.wrapping_sub(right);

        self.A = val;

        let z: bool = val == 0;
        let n: bool = true;
        // Cは影響を受けない
        let (h, _) = self.is_carry_negative(left, right as u16);
        let c = self.get_carry_flag();

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn inc_34(&mut self) -> Result<u8> {
        let address = self.get_hl();
        let left = self.bus.read(address)?;
        let right = 1;
        let val = left.wrapping_add(right);

        self.bus.write(address, val)?;

        let z: bool = val == 0;
        let n: bool = false;
        // Cは影響を受けない
        let (h, _) = self.is_carry_positive(left as u16, right as u16);
        let c = self.get_carry_flag();

        self.set_flag(z, n, h, c);
        Ok(12)
    }

    #[allow(dead_code)]
    fn inc_2C(&mut self) -> Result<u8> {
        let left = self.L;
        let right = 1;
        let val = left.wrapping_add(right);

        self.L = val;

        let z: bool = val == 0;
        let n: bool = false;
        // Cは影響を受けない
        let (h, _) = self.is_carry_positive(left as u16, right as u16);
        let c = self.get_carry_flag();

        self.set_flag(z, n, h, c);
        Ok(4)
    }
    
    #[allow(dead_code)]
    fn inc_24(&mut self) -> Result<u8> {
        let left = self.H;
        let right = 1;
        let val = left.wrapping_add(right);

        self.H = val;

        let z: bool = val == 0;
        let n: bool = false;
        // Cは影響を受けない
        let (h, _) = self.is_carry_positive(left as u16, right as u16);
        let c = self.get_carry_flag();

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn inc_1C(&mut self) -> Result<u8> {
        let left = self.E;
        let right = 1;
        let val = left.wrapping_add(right);

        self.E = val;

        let z: bool = val == 0;
        let n: bool = false;
        // Cは影響を受けない
        let (h, _) = self.is_carry_positive(left as u16, right as u16);
        let c = self.get_carry_flag();

        self.E = val;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn inc_14(&mut self) -> Result<u8> {
        let left = self.D;
        let right = 1;
        let val = left.wrapping_add(right);

        self.D = val;

        let z: bool = val == 0;
        let n: bool = false;
        // Cは影響を受けない
        let (h, _) = self.is_carry_positive(left as u16, right as u16);
        let c = self.get_carry_flag();

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn inc_0C(&mut self) -> Result<u8> {
        let left = self.C;
        let right = 1;
        let val = left.wrapping_add(right);

        self.C = val;

        let z: bool = val == 0;
        let n: bool = false;
        // Cは影響を受けない
        let (h, _) = self.is_carry_positive(left as u16, right as u16);
        let c = self.get_carry_flag();

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn inc_04(&mut self) -> Result<u8> {
        let left = self.B;
        let right = 1;
        let val = left.wrapping_add(right);

        self.B = val;

        let z: bool = val == 0;
        let n: bool = false;
        // Cは影響を受けない
        let (h, _) = self.is_carry_positive(left as u16, right as u16);
        let c = self.get_carry_flag();

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn inc_3C(&mut self) -> Result<u8> {
        let left = self.A;
        let right = 1;
        let val = left.wrapping_add(right);

        self.A = val;

        let z: bool = val == 0;
        let n: bool = false;
        // Cは影響を受けない
        let (h, _) = self.is_carry_positive(left as u16, right as u16);
        let c = self.get_carry_flag();

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn cp_FE(&mut self) -> Result<u8> {
        let left = self.A;
        let right = self.read_next_8()?;
        let val = left.wrapping_sub(right);

        let z: bool = val == 0;
        let n: bool = true;
        let (h, c) = self.is_carry_negative(left, right as u16);

        self.set_flag(z, n, h, c);
        Ok(8)
    }

    #[allow(dead_code)]
    fn cp_BE(&mut self) -> Result<u8> {
        let address = self.get_hl();
        let left = self.A;
        let right = self.bus.read(address)?;
        let val = left.wrapping_sub(right);

        let z: bool = val == 0;
        let n: bool = true;
        let (h, c) = self.is_carry_negative(left, right as u16);

        self.set_flag(z, n, h, c);
        Ok(8)
    }

    #[allow(dead_code)]
    fn cp_BD(&mut self) -> Result<u8> {
        let left = self.A;
        let right = self.L;
        let val = left.wrapping_sub(right);

        let z: bool = val == 0;
        let n: bool = true;
        let (h, c) = self.is_carry_negative(left, right as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn cp_BC(&mut self) -> Result<u8> {
        let left = self.A;
        let right = self.H;
        let val = left.wrapping_sub(right);

        let z: bool = val == 0;
        let n: bool = true;
        let (h, c) = self.is_carry_negative(left, right as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn cp_BB(&mut self) -> Result<u8> {
        let left = self.A;
        let right = self.E;
        let val = left.wrapping_sub(right);

        let z: bool = val == 0;
        let n: bool = true;
        let (h, c) = self.is_carry_negative(left, right as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn cp_BA(&mut self) -> Result<u8> {
        let left = self.A;
        let right = self.D;
        let val = left.wrapping_sub(right);

        let z: bool = val == 0;
        let n: bool = true;
        let (h, c) = self.is_carry_negative(left, right as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn cp_B9(&mut self) -> Result<u8> {
        let left = self.A;
        let right = self.C;
        let val = left.wrapping_sub(right);

        let z: bool = val == 0;
        let n: bool = true;
        let (h, c) = self.is_carry_negative(left, right as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn cp_B8(&mut self) -> Result<u8> {
        let left = self.A;
        let right = self.B;
        let val = left.wrapping_sub(right);

        let z: bool = val == 0;
        let n: bool = true;
        let (h, c) = self.is_carry_negative(left, right as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn cp_BF(&mut self) -> Result<u8> {
        let left = self.A;
        let right = self.A;
        let val = left.wrapping_sub(right);

        let z: bool = val == 0;
        let n: bool = true;
        let (h, c) = self.is_carry_negative(left, right as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn xor_EE(&mut self) -> Result<u8> {
        let data = self.read_next_8()?;
        let val = self.A ^ data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = false;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(8)
    }

    #[allow(dead_code)]
    fn xor_AE(&mut self) -> Result<u8> {
        let address: u16 = self.get_hl();
        let data = self.bus.read(address)?;
        let val = self.A ^ data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = false;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(8)
    }

    #[allow(dead_code)]
    fn xor_AD(&mut self) -> Result<u8> {
        let data = self.L;
        let val = self.A ^ data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = false;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn xor_AC(&mut self) -> Result<u8> {
        let data = self.H;
        let val = self.A ^ data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = false;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn xor_AB(&mut self) -> Result<u8> {
        let data = self.E;
        let val = self.A ^ data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = false;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn xor_AA(&mut self) -> Result<u8> {
        let data = self.D;
        let val = self.A ^ data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = false;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn xor_A9(&mut self) -> Result<u8> {
        let data = self.C;
        let val = self.A ^ data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = false;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn xor_A8(&mut self) -> Result<u8> {
        let data = self.B;
        let val = self.A ^ data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = false;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn xor_AF(&mut self) -> Result<u8> {
        let data = self.A;
        let val = self.A ^ data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = false;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn or_F6(&mut self) -> Result<u8> {
        let data: u8 = self.read_next_8()?;
        let val = self.A | data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = false;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(8)
    }

    #[allow(dead_code)]
    fn or_B6(&mut self) -> Result<u8> {
        let address: u16 = self.get_hl();
        let data: u8 = self.bus.read(address)?;
        let val = self.A | data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = false;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(8)
    }

    #[allow(dead_code)]
    fn or_B5(&mut self) -> Result<u8> {
        let data: u8 = self.L;
        let val = self.A | data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = false;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn or_B4(&mut self) -> Result<u8> {
        let data: u8 = self.H;
        let val = self.A | data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = false;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn or_B3(&mut self) -> Result<u8> {
        let data: u8 = self.E;
        let val = self.A | data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = false;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn or_B2(&mut self) -> Result<u8> {
        let data: u8 = self.D;
        let val = self.A | data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = false;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn or_B1(&mut self) -> Result<u8> {
        let data: u8 = self.C;
        let val = self.A | data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = false;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn or_B0(&mut self) -> Result<u8> {
        let data: u8 = self.B;
        let val = self.A | data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = false;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn or_B7(&mut self) -> Result<u8> {
        let data: u8 = self.A;
        let val = self.A | data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = false;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn and_E6(&mut self) -> Result<u8> {
        let data: u8 = self.read_next_8()?;
        let val = self.A & data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = true;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(8)
    }

    #[allow(dead_code)]
    fn and_A6(&mut self) -> Result<u8> {
        let address: u16 = self.get_hl();
        let data: u8 = self.bus.read(address)?;
        let val = self.A & data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = true;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(8)
    }

    #[allow(dead_code)]
    fn and_A5(&mut self) -> Result<u8> {
        let data: u8 = self.L;
        let val = self.A & data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = true;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn and_A4(&mut self) -> Result<u8> {
        let data: u8 = self.H;
        let val = self.A & data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = true;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn and_A3(&mut self) -> Result<u8> {
        let data: u8 = self.E;
        let val = self.A & data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = true;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn and_A2(&mut self) -> Result<u8> {
        let data: u8 = self.D;
        let val = self.A & data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = true;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn and_A1(&mut self) -> Result<u8> {
        let data: u8 = self.C;
        let val = self.A & data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = true;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn and_A0(&mut self) -> Result<u8> {
        let data: u8 = self.B;
        let val = self.A & data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = true;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn and_A7(&mut self) -> Result<u8> {
        let data: u8 = self.A;
        let val = self.A & data;
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let h: bool = true;
        let c: bool = false;

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    fn base_sbc(&mut self, left: u8, carry_val: u8, data: u8, is_hl: bool) -> Result<u8> {
        let val = left.wrapping_sub(carry_val).wrapping_sub(data);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = true;
        let (h, c) = self.is_carry_negative(left, data as u16 + carry_val as u16);

        self.set_flag(z, n, h, c);

        if is_hl {
            Ok(8)
        }
        else {
            Ok(4)
        }
    }

    #[allow(dead_code)]
    fn sbc_9E(&mut self) -> Result<u8> {
        let left = self.A;
        let address: u16 = self.get_hl();
        let data: u8 = self.bus.read(address)?;
        let cf: bool = self.get_carry_flag();
        let carry_val: u8 = if cf { 1 } else { 0 };

        self.base_sbc(left, carry_val, data, true)
    }

    #[allow(dead_code)]
    fn sbc_9D(&mut self) -> Result<u8> {
        let left = self.A;
        let data: u8 = self.L;
        let cf: bool = self.get_carry_flag();
        let carry_val: u8 = if cf { 1 } else { 0 };

        self.base_sbc(left, carry_val, data, false)
    }

    #[allow(dead_code)]
    fn sbc_9C(&mut self) -> Result<u8> {
        let left = self.A;
        let data: u8 = self.H;
        let cf: bool = self.get_carry_flag();
        let carry_val: u8 = if cf { 1 } else { 0 };

        self.base_sbc(left, carry_val, data, false)
    }

    #[allow(dead_code)]
    fn sbc_9B(&mut self) -> Result<u8> {
        let left = self.A;
        let data: u8 = self.E;
        let cf: bool = self.get_carry_flag();
        let carry_val: u8 = if cf { 1 } else { 0 };

        self.base_sbc(left, carry_val, data, false)
    }

    #[allow(dead_code)]
    fn sbc_9A(&mut self) -> Result<u8> {
        let left = self.A;
        let data: u8 = self.D;
        let cf: bool = self.get_carry_flag();
        let carry_val: u8 = if cf { 1 } else { 0 };

        self.base_sbc(left, carry_val, data, false)
    }

    #[allow(dead_code)]
    fn sbc_99(&mut self) -> Result<u8> {
        let left = self.A;
        let data: u8 = self.C;
        let cf: bool = self.get_carry_flag();
        let carry_val: u8 = if cf { 1 } else { 0 };

        self.base_sbc(left, carry_val, data, false)
    }

    #[allow(dead_code)]
    fn sbc_98(&mut self) -> Result<u8> {
        let left = self.A;
        let data: u8 = self.B;
        let cf: bool = self.get_carry_flag();
        let carry_val: u8 = if cf { 1 } else { 0 };

        self.base_sbc(left, carry_val, data, false)
    }

    #[allow(dead_code)]
    fn sbc_9F(&mut self) -> Result<u8> {
        let left = self.A;
        let data: u8 = self.A;
        let cf: bool = self.get_carry_flag();
        let carry_val: u8 = if cf { 1 } else { 0 };

        self.base_sbc(left, carry_val, data, false)
    }

    #[allow(dead_code)]
    fn sub_D6(&mut self) -> Result<u8> {
        let left = self.A;
        let data: u8 = self.read_next_8()?;
        let val = left.wrapping_sub(data);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = true;
        let (h, c) = self.is_carry_negative(left, data as u16);

        self.set_flag(z, n, h, c);
        Ok(8)
    }

    #[allow(dead_code)]
    fn sub_96(&mut self) -> Result<u8> {
        let left = self.A;
        let address = self.get_hl();
        let data: u8 = self.bus.read(address)?;
        let val = left.wrapping_sub(data);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = true;
        let (h, c) = self.is_carry_negative(left, data as u16);

        self.set_flag(z, n, h, c);
        Ok(8)
    }

    #[allow(dead_code)]
    fn sub_95(&mut self) -> Result<u8> {
        let left = self.A;
        let data: u8 = self.L;
        let val = left.wrapping_sub(data);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = true;
        let (h, c) = self.is_carry_negative(left, data as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn sub_94(&mut self) -> Result<u8> {
        let left = self.A;
        let data: u8 = self.H;
        let val = left.wrapping_sub(data);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = true;
        let (h, c) = self.is_carry_negative(left, data as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn sub_93(&mut self) -> Result<u8> {
        let left = self.A;
        let data: u8 = self.E;
        let val = left.wrapping_sub(data);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = true;
        let (h, c) = self.is_carry_negative(left, data as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn sub_92(&mut self) -> Result<u8> {
        let left = self.A;
        let data: u8 = self.D;
        let val = left.wrapping_sub(data);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = true;
        let (h, c) = self.is_carry_negative(left, data as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }
    
    #[allow(dead_code)]
    fn sub_91(&mut self) -> Result<u8> {
        let left = self.A;
        let data: u8 = self.C;
        let val = left.wrapping_sub(data);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = true;
        let (h, c) = self.is_carry_negative(left, data as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn sub_90(&mut self) -> Result<u8> {
        let left = self.A;
        let data: u8 = self.B;
        let val = left.wrapping_sub(data);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = true;
        let (h, c) = self.is_carry_negative(left, data as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn sub_97(&mut self) -> Result<u8> {
        let left = self.A;
        let data: u8 = self.A;
        let val = left.wrapping_sub(data);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = true;
        let (h, c) = self.is_carry_negative(left, data as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn adc_CE(&mut self) -> Result<u8> {
        let left = self.A;
        let data: u8 = self.read_next_8()?;
        let cf: bool = self.get_carry_flag();
        let carry_val: u8 = if cf { 1 } else { 0 };

        let val = left.wrapping_add(data).wrapping_add(carry_val);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let (h, c) = self.is_carry_positive(left as u16, data as u16 + carry_val as u16);

        self.set_flag(z, n, h, c);
        Ok(8)
    }

    #[allow(dead_code)]
    fn adc_8E(&mut self) -> Result<u8> {
        let left = self.A;
        let address: u16 = self.get_hl();
        let data: u8 = self.bus.read(address)?;
        let cf: bool = self.get_carry_flag();
        let carry_val: u8 = if cf { 1 } else { 0 };

        let val = left.wrapping_add(data).wrapping_add(carry_val);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let (h, c) = self.is_carry_positive(left as u16, data as u16 + carry_val as u16);

        self.set_flag(z, n, h, c);
        Ok(8)
    }

    #[allow(dead_code)]
    fn adc_8D(&mut self) -> Result<u8> {
        let left = self.A;
        let right: u8 = self.L;
        let cf: bool = self.get_carry_flag();
        let carry_val: u8 = if cf { 1 } else { 0 };

        let val = left.wrapping_add(right).wrapping_add(carry_val);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let (h, c) = self.is_carry_positive(left as u16, right as u16 + carry_val as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn adc_8C(&mut self) -> Result<u8> {
        let left = self.A;
        let right: u8 = self.H;
        let cf: bool = self.get_carry_flag();
        let carry_val: u8 = if cf { 1 } else { 0 };

        let val = left.wrapping_add(right).wrapping_add(carry_val);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let (h, c) = self.is_carry_positive(left as u16, right as u16 + carry_val as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn adc_8B(&mut self) -> Result<u8> {
        let left = self.A;
        let right: u8 = self.E;
        let cf: bool = self.get_carry_flag();
        let carry_val: u8 = if cf { 1 } else { 0 };

        let val = left.wrapping_add(right).wrapping_add(carry_val);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let (h, c) = self.is_carry_positive(left as u16, right as u16 + carry_val as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn adc_8A(&mut self) -> Result<u8> {
        let left = self.A;
        let right: u8 = self.D;
        let cf: bool = self.get_carry_flag();
        let carry_val: u8 = if cf { 1 } else { 0 };

        let val = left.wrapping_add(right).wrapping_add(carry_val);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let (h, c) = self.is_carry_positive(left as u16, right as u16 + carry_val as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn adc_89(&mut self) -> Result<u8> {
        let left = self.A;
        let right: u8 = self.C;
        let cf: bool = self.get_carry_flag();
        let carry_val: u8 = if cf { 1 } else { 0 };

        let val = left.wrapping_add(right).wrapping_add(carry_val);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let (h, c) = self.is_carry_positive(left as u16, right as u16 + carry_val as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn adc_88(&mut self) -> Result<u8> {
        let left = self.A;
        let right: u8 = self.B;
        let cf: bool = self.get_carry_flag();
        let carry_val: u8 = if cf { 1 } else { 0 };

        let val = left.wrapping_add(right).wrapping_add(carry_val);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let (h, c) = self.is_carry_positive(left as u16, right as u16 + carry_val as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn adc_8F(&mut self) -> Result<u8> {
        let left = self.A;
        let right: u8 = self.A;
        let cf: bool = self.get_carry_flag();
        let carry_val: u8 = if cf { 1 } else { 0 };

        let val = left.wrapping_add(right).wrapping_add(carry_val);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let (h, c) = self.is_carry_positive(left as u16, right as u16 + carry_val as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn add_C6(&mut self) -> Result<u8> {
        let left = self.A;
        let right: u8 = self.read_next_8()?;
        let val = left.wrapping_add(right);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let (h, c) = self.is_carry_positive(left as u16, right as u16);

        self.set_flag(z, n, h, c);
        Ok(8)
    }

    #[allow(dead_code)]
    fn add_86(&mut self) -> Result<u8> {
        let left = self.A;
        let hl = self.get_hl();
        let right: u8 = self.bus.read(hl)?;
        let val = left.wrapping_add(right);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let (h, c) = self.is_carry_positive(left as u16, right as u16);

        self.set_flag(z, n, h, c);
        Ok(8)
    }

    #[allow(dead_code)]
    fn add_85(&mut self) -> Result<u8> {
        let left = self.A;
        let right: u8 = self.L;
        let val = left.wrapping_add(right);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let (h, c) = self.is_carry_positive(left as u16, right as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn add_84(&mut self) -> Result<u8> {
        let left = self.A;
        let right: u8 = self.H;
        let val = left.wrapping_add(right);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let (h, c) = self.is_carry_positive(left as u16, right as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn add_83(&mut self) -> Result<u8> {
        let left = self.A;
        let right: u8 = self.E;
        let val = left.wrapping_add(right);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let (h, c) = self.is_carry_positive(left as u16, right as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn add_82(&mut self) -> Result<u8> {
        let left = self.A;
        let right: u8 = self.D;
        let val = left.wrapping_add(right);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let (h, c) = self.is_carry_positive(left as u16, right as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn add_81(&mut self) -> Result<u8> {
        let left = self.A;
        let right: u8 = self.C;
        let val = left.wrapping_add(right);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let (h, c) = self.is_carry_positive(left as u16, right as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn add_80(&mut self) -> Result<u8> {
        let left = self.A;
        let right: u8 = self.B;
        let val = left.wrapping_add(right);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let (h, c) = self.is_carry_positive(left as u16, right as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn add_87(&mut self) -> Result<u8> {
        let left = self.A;
        let right: u8 = self.A;

        let val = left.wrapping_add(right);
        self.A = val;

        let z: bool = self.A == 0;
        let n: bool = false;
        let (h, c) = self.is_carry_positive(left as u16, right as u16);

        self.set_flag(z, n, h, c);
        Ok(4)
    }

    #[allow(dead_code)]
    fn pop_E1(&mut self) -> Result<u8> {
        let address: u16 = self.SP;
        let data: u16 = self.bus.read_16(address)?;
        self.set_hl(data);
        
        // 二回インクリメントする
        self.increment_sp();
        self.increment_sp();

        Ok(12)
    }

    #[allow(dead_code)]
    fn pop_D1(&mut self) -> Result<u8> {
        let address: u16 = self.SP;
        let data: u16 = self.bus.read_16(address)?;
        self.set_de(data);
        
        // 二回インクリメントする
        self.increment_sp();
        self.increment_sp();

        Ok(12)
    }

    #[allow(dead_code)]
    fn pop_C1(&mut self) -> Result<u8> {
        let address: u16 = self.SP;
        let data: u16 = self.bus.read_16(address)?;
        self.set_bc(data);
        
        // 二回インクリメントする
        self.increment_sp();
        self.increment_sp();

        Ok(12)
    }

    #[allow(dead_code)]
    fn pop_F1(&mut self) -> Result<u8> {
        let address: u16 = self.SP;
        let data: u16 = self.bus.read_16(address)?;
        self.set_af(data & 0xFFF0);
        
        // 二回インクリメントする
        self.increment_sp();
        self.increment_sp();

        Ok(12)
    }

    #[allow(dead_code)]
    fn push_E5(&mut self) -> Result<u8> {
        // 二回デクリメントする
        self.decrement_sp();
        self.decrement_sp();

        let data: u16 = self.get_hl();
        let address: u16 = self.SP;
        self.bus.write_16(address, data)?;

        Ok(16)
    }

    #[allow(dead_code)]
    fn push_D5(&mut self) -> Result<u8> {
        // 二回デクリメントする
        self.decrement_sp();
        self.decrement_sp();

        let data: u16 = self.get_de();
        let address: u16 = self.SP;
        self.bus.write_16(address, data)?;

        Ok(16)
    }

    #[allow(dead_code)]
    fn push_C5(&mut self) -> Result<u8> {
        // 二回デクリメントする
        self.decrement_sp();
        self.decrement_sp();

        let data: u16 = self.get_bc();
        let address: u16 = self.SP;
        self.bus.write_16(address, data)?;

        Ok(16)
    }

    #[allow(dead_code)]
    fn push_F5(&mut self) -> Result<u8> {
        // 二回デクリメントする
        self.decrement_sp();
        self.decrement_sp();

        let data: u16 = self.get_af();
        let address: u16 = self.SP;
        self.bus.write_16(address, data)?;

        Ok(16)
    }

    #[allow(dead_code)]
    fn ld_08(&mut self) -> Result<u8> {
        let address: u16 = self.read_next_16()?;
        let data: u16 = self.SP;
        self.bus.write_16(address, data)?;

        Ok(20)
    }

    #[allow(dead_code)]
    fn ld_F8(&mut self) -> Result<u8> {
        let input = self.read_next_8()? as i8;
        let address = self.SP as isize + input as isize;
        self.set_hl(address as u16);

        let z: bool = false;
        let n: bool = false;
        let (h, c) = self.is_carry_positive(self.SP, input as u16);

        self.set_flag(z, n, h, c);

        Ok(12)
    }

    #[allow(dead_code)]
    fn ld_F9(&mut self) -> Result<u8> {
        let hl = self.get_hl();
        self.SP = hl;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_31(&mut self) -> Result<u8> {
        let data: u16 = self.read_next_16()?;
        self.SP = data;

        Ok(12)
    }

    #[allow(dead_code)]
    fn ld_21(&mut self) -> Result<u8> {
        let data: u16 = self.read_next_16()?;
        self.set_hl(data);

        Ok(12)
    }

    #[allow(dead_code)]
    fn ld_11(&mut self) -> Result<u8> {
        let data: u16 = self.read_next_16()?;
        self.set_de(data);

        Ok(12)
    }

    #[allow(dead_code)]
    fn ld_01(&mut self) -> Result<u8> {
        let data: u16 = self.read_next_16()?;
        self.set_bc(data);

        Ok(12)
    }

    #[allow(dead_code)]
    fn ld_F0(&mut self) -> Result<u8> {
        let input: u8 = self.read_next_8()?;
        let address = (input as u16) + (0xFF00);
        let data = self.bus.read(address)?;
        
        self.A = data;

        Ok(12)
    }

    #[allow(dead_code)]
    fn ld_E0(&mut self) -> Result<u8> {
        let input: u8 = self.read_next_8()?;
        let address = (input as u16) + (0xFF00);
        let data = self.A;
        self.bus.write(address, data)?;

        Ok(12)
    }

    #[allow(dead_code)]
    fn ld_22(&mut self) -> Result<u8> {
        let address: u16 = self.get_hl();
        let data = self.A;
        self.bus.write(address, data)?;
        self.increment_hl();

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_2A(&mut self) -> Result<u8> {
        let address: u16 = self.get_hl();
        let data = self.bus.read(address)?;
        self.A = data;
        self.increment_hl();

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_32(&mut self) -> Result<u8> {
        let address: u16 = self.get_hl();
        let data = self.A;
        self.bus.write(address, data)?;
        self.decrement_hl();

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_3A(&mut self) -> Result<u8> {
        let address: u16 = self.get_hl();
        let data = self.bus.read(address)?;
        self.A = data;
        self.decrement_hl();

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_E2(&mut self) -> Result<u8> {
        let address: u16 = (0xFF00 as u16) + (self.C as u16);
        let data: u8 = self.A;
        self.bus.write(address, data)?;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_F2(&mut self) -> Result<u8> {
        let address: u16 = (0xFF00 as u16) + (self.C as u16);
        let data: u8 = self.bus.read(address)?;
        self.A = data;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_EA(&mut self) -> Result<u8> {
        let address = self.read_next_16()?;
        self.bus.write(address, self.A)?;

        Ok(16)
    }

    #[allow(dead_code)]
    fn ld_77(&mut self) -> Result<u8> {
        let hl = self.get_hl();
        self.bus.write(hl, self.A)?;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_12(&mut self) -> Result<u8> {
        let de = self.get_de();
        self.bus.write(de, self.A)?;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_02(&mut self) -> Result<u8> {
        let bc = self.get_bc();
        self.bus.write(bc, self.A)?;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_6F(&mut self) -> Result<u8> {
        self.L = self.A;

        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_67(&mut self) -> Result<u8> {
        self.H = self.A;

        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_5F(&mut self) -> Result<u8> {
        self.E = self.A;

        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_57(&mut self) -> Result<u8> {
        self.D = self.A;

        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_4F(&mut self) -> Result<u8> {
        self.C = self.A;

        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_47(&mut self) -> Result<u8> {
        self.B = self.A;

        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_3E(&mut self) -> Result<u8> {
        let data = self.read_next_8()?;
        self.A = data;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_FA(&mut self) -> Result<u8> {
        // 16bitを読み込む
        // read_16内でPCはインクリメントされる
        let address = self.read_next_16()?;

        let data = self.bus.read(address)?;
        self.A = data;

        Ok(16)
    }
    
    #[allow(dead_code)]
    fn ld_1A(&mut self) -> Result<u8> {
        let de = self.get_de();
        let data = self.bus.read(de)?;
        self.A = data;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_0A(&mut self) -> Result<u8> {
        let bc = self.get_bc();
        let data = self.bus.read(bc)?;
        self.A = data;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_36(&mut self) -> Result<u8> {
        let hl = self.get_hl();

        self.increment_pc();
        let data = self.bus.read(self.PC)?;

        self.bus.write(hl, data)?;

        Ok(12)
    }

    #[allow(dead_code)]
    fn ld_75(&mut self) -> Result<u8> {
        let hl = self.get_hl();
        let data = self.L;

        self.bus.write(hl, data)?;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_74(&mut self) -> Result<u8> {
        let hl = self.get_hl();
        let data = self.H;

        self.bus.write(hl, data)?;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_73(&mut self) -> Result<u8> {
        let hl = self.get_hl();
        let data = self.E;

        self.bus.write(hl, data)?;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_72(&mut self) -> Result<u8> {
        let hl = self.get_hl();
        let data = self.D;

        self.bus.write(hl, data)?;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_71(&mut self) -> Result<u8> {
        let hl = self.get_hl();
        let data = self.C;

        self.bus.write(hl, data)?;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_70(&mut self) -> Result<u8> {
        let hl = self.get_hl();
        let data = self.B;

        self.bus.write(hl, data)?;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_6E(&mut self) -> Result<u8> {
        let hl: u16 = self.get_hl();
        if let Ok(res) = self.bus.read(hl) {
            self.L = res;
        }
        else {
            bail!("fail! error occured in ld_6E")
        }

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_6D(&mut self) -> Result<u8> {
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_6C(&mut self) -> Result<u8> {
        self.L = self.H;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_6B(&mut self) -> Result<u8> {
        self.L = self.E;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_6A(&mut self) -> Result<u8> {
        self.L = self.D;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_69(&mut self) -> Result<u8> {
        self.L = self.C;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_68(&mut self) -> Result<u8> {
        self.L = self.B;
        Ok(4)
    }
    
    #[allow(dead_code)]
    fn ld_66(&mut self) -> Result<u8> {
        let hl: u16 = self.get_hl();
        if let Ok(res) = self.bus.read(hl) {
            self.H = res;
        }
        else {
            bail!("fail! error occured in ld_66")
        }

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_65(&mut self) -> Result<u8> {
        self.H = self.L;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_64(&mut self) -> Result<u8> {
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_63(&mut self) -> Result<u8> {
        self.H = self.E;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_62(&mut self) -> Result<u8> {
        self.H = self.D;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_61(&mut self) -> Result<u8> {
        self.H = self.C;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_60(&mut self) -> Result<u8> {
        self.H = self.B;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_5E(&mut self) -> Result<u8> {
        let hl: u16 = self.get_hl();
        if let Ok(res) = self.bus.read(hl) {
            self.E = res;
        }
        else {
            bail!("fail! error occured in ld_5E")
        }

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_5D(&mut self) -> Result<u8> {
        self.E = self.L;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_5C(&mut self) -> Result<u8> {
        self.E = self.H;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_5B(&mut self) -> Result<u8> {
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_5A(&mut self) -> Result<u8> {
        self.E = self.D;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_59(&mut self) -> Result<u8> {
        self.E = self.C;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_58(&mut self) -> Result<u8> {
        self.E = self.B;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_56(&mut self) -> Result<u8> {
        let hl: u16 = self.get_hl();
        if let Ok(res) = self.bus.read(hl) {
            self.D = res;
        }
        else {
            bail!("fail! error occured in ld_56")
        }

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_55(&mut self) -> Result<u8> {
        self.D = self.L;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_54(&mut self) -> Result<u8> {
        self.D = self.H;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_53(&mut self) -> Result<u8> {
        self.D = self.E;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_52(&mut self) -> Result<u8> {
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_51(&mut self) -> Result<u8> {
        self.D = self.C;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_50(&mut self) -> Result<u8> {
        self.D = self.B;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_4E(&mut self) -> Result<u8> {
        let hl: u16 = self.get_hl();
        if let Ok(res) = self.bus.read(hl) {
            self.C = res;
        }
        else {
            bail!("fail! error occured in ld_4E")
        }

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_4D(&mut self) -> Result<u8> {
        self.C = self.L;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_4C(&mut self) -> Result<u8> {
        self.C = self.H;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_4B(&mut self) -> Result<u8> {
        self.C = self.E;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_4A(&mut self) -> Result<u8> {
        self.C = self.D;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_49(&mut self) -> Result<u8> {
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_48(&mut self) -> Result<u8> {
        self.C = self.B;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_46(&mut self) -> Result<u8> {
        let hl: u16 = self.get_hl();
        if let Ok(res) = self.bus.read(hl) {
            self.B = res;
        }
        else {
            bail!("fail! error occured in ld_46")
        }

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_45(&mut self) -> Result<u8> {
        self.B = self.L;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_44(&mut self) -> Result<u8> {
        self.B = self.H;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_43(&mut self) -> Result<u8> {
        self.B = self.E;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_42(&mut self) -> Result<u8> {
        self.B = self.D;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_41(&mut self) -> Result<u8> {
        self.B = self.C;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_40(&mut self) -> Result<u8> {
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_06(&mut self) -> Result<u8> {
        let val = self.read_next_8()?;
        self.B = val;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_0E(&mut self) -> Result<u8> {
        let val = self.read_next_8()?;
        self.C = val;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_16(&mut self) -> Result<u8> {
        let val = self.read_next_8()?;
        self.D = val;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_1E(&mut self) -> Result<u8> {
        let val = self.read_next_8()?;
        self.E = val;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_26(&mut self) -> Result<u8> {
        let val = self.read_next_8()?;
        self.H = val;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_2E(&mut self) -> Result<u8> {
        let val = self.read_next_8()?;
        self.L = val;

        Ok(8)
    }

    #[allow(dead_code)]
    fn ld_7F(&mut self) -> Result<u8> {
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_78(&mut self) -> Result<u8> {
        self.A = self.B;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_79(&mut self) -> Result<u8> {
        self.A = self.C;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_7A(&mut self) -> Result<u8> {
        self.A = self.D;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_7B(&mut self) -> Result<u8> {
        self.A = self.E;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_7C(&mut self) -> Result<u8> {
        self.A = self.H;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_7D(&mut self) -> Result<u8> {
        self.A = self.L;
        Ok(4)
    }

    #[allow(dead_code)]
    fn ld_7E(&mut self) -> Result<u8> {
        let hl: u16 = self.get_hl();
        if let Ok(res) = self.bus.read(hl) {
            self.A = res;
        }
        else {
            bail!("fail! error occured in ld_7E")
        }

        Ok(8)
    }

    // endregion: inst
}