use anyhow::Result;
use cpal;
use dasp::{Signal, Sample, self as signal, ring_buffer, frame::Stereo};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

#[derive(Clone, Copy, Debug, Default)]
pub struct Ch1 {
    sweep_period: u8,
    sweep_timer: u8,
    sweep_down: bool,
    calc_sweep_in_neg: bool,
    sweep_shift: u8,
    sweep_flag: bool,
    shadow_frequency: u16,
    length: u8,
    length_timer: u8,
    length_ticking: bool,
    stop_flag: bool,
    duty_pattern: u8,
    env_initial_volume: u8,
    env_up: bool,
    env_period: u8,
    env_timer: u8,
    frequency: u16,
    duty_position: u8,
    frequency_timer: u16,
    volume: u8,
    channel_on: bool
}

impl Ch1 {
    pub fn read(&self, address: u16) -> Result<u8> {
        let data = match address {
            0 => {
                ((self.sweep_period << 4) + ((self.sweep_down as u8) << 3) + self.sweep_shift) | 0b10000000
            },
            1 => {
                (self.duty_pattern << 6) | 0b111111
            },
            2 => {
                (self.env_initial_volume << 4) + ((self.env_up as u8) << 3) + (self.env_period)
            },
            3 => {
                0xFF
            },
            4 => {
                ((self.stop_flag as u8) << 6) | 0b10111111
            },
            _ => 0xFF
        };

        Ok(data)
    }

    pub fn write(&mut self, address: u16, data: u8) -> Result<()> {
        match address {
            0 => {
                let prev_sweep_down = self.sweep_down;
                self.sweep_period = (data >> 4) & 0b111;
                self.sweep_down = (data & (1 << 3)) > 0;
                self.sweep_shift = data & 0b111;

                if prev_sweep_down && self.calc_sweep_in_neg && !self.sweep_down {
                    self.channel_on = false;
                    self.calc_sweep_in_neg = false;
                }
            },
            1 => {
                self.duty_pattern = data >> 6;
                self.length = data & 63;
                self.length_timer = 64 - self.length;
            },
            2 => {
                self.env_initial_volume = data >> 4;
                self.env_up = (data & (1 << 3)) > 0;
                self.env_period = data & 0b111;
                self.channel_on = self.channel_on & self.dac_enable();
            },
            3 => {
                self.frequency = (self.frequency - (self.frequency & 0xFF)) + data as u16;
            },
            4 => {
                let freq_upper_bit = (data & 0b111) as u16;
                self.frequency = (self.frequency & 0xFF) + (freq_upper_bit << 8);

                let prev_stop_flag = self.stop_flag;
                self.stop_flag = (data & (1 << 6)) > 0;

                if !prev_stop_flag && self.stop_flag && self.length_ticking {
                    self.length();
                }
                
                if (data & (1 << 7)) > 0 {
                    self.trigger();
                }
            },
            _ => {}
        }

        Ok(())
    }

    fn dac_enable(&self) -> bool {
        return (self.env_initial_volume > 0) || self.env_up;
    }

    fn trigger(&mut self) {
        self.channel_on = self.dac_enable();
        if self.length_timer == 0 {
            self.length_timer = 64;
            if self.stop_flag && self.length_ticking {
                self.length();
            }
        }
        self.frequency_timer = (2048 - self.frequency) * 4;
        self.env_timer = self.env_period;
        self.volume = self.env_initial_volume;

        self.shadow_frequency = self.frequency;
        self.sweep_timer = if self.sweep_period == 0 {
            8
        }
        else {
            self.sweep_period
        };

        self.calc_sweep_in_neg = false;
        self.sweep_flag = self.sweep_period > 0 || self.sweep_shift > 0;
        self.shadow_frequency = self.frequency;
        if self.sweep_shift > 0 {
            if self.calc_new_frequency() > 2047 {
                self.channel_on = false;
            }
        }
    }

    fn frequency_tick(&mut self) {
        self.frequency_timer = self.frequency_timer.wrapping_sub(1);

        if self.frequency_timer == 0 {
            self.frequency_timer = (2048 - self.frequency) * 4;
            self.duty_position += 1;
            self.duty_position %= 8;
        }
    }

    fn envelope(&mut self) {
        if self.env_period != 0 {
            if self.env_timer > 0 {
                self.env_timer = self.env_timer.wrapping_sub(1);
            }

            if self.env_timer == 0 {
                self.env_timer = self.env_period;

                // 音を上げる
                if self.env_up {
                    if self.volume < 15 {
                        self.volume = self.volume.wrapping_add(1);
                    }
                }
                else {
                    if self.volume > 0 {
                        self.volume = self.volume.wrapping_sub(1);
                    }
                }
            }
        }
    }

    fn sweep(&mut self) {
        if self.sweep_timer > 0 {
            self.sweep_timer = self.sweep_timer.wrapping_sub(1);
        }

        if self.sweep_timer == 0 {
            self.sweep_timer = if self.sweep_period == 0 {
                8
            }
            else {
                self.sweep_period
            };

            if self.sweep_flag && self.sweep_period > 0 {
                let new_frequency = self.calc_new_frequency();

                if new_frequency <= 2047 && self.sweep_shift > 0 {
                    self.frequency = new_frequency;
                    self.shadow_frequency = new_frequency;
                }

                if self.calc_new_frequency() > 2047 {
                    self.channel_on = false;
                }
            }
        }
    }

    fn calc_new_frequency(&mut self) -> u16 {
        let offset = self.shadow_frequency >> self.sweep_shift;
        if !self.sweep_down {
            return self.shadow_frequency.wrapping_add(offset)
        }

        self.calc_sweep_in_neg = true;
        return self.shadow_frequency.wrapping_sub(offset)
    }

    fn length(&mut self) {
        if !self.stop_flag || self.length_timer == 0 {
            return;
        }

        self.length_timer = self.length_timer.wrapping_sub(1);
        if self.length_timer == 0 {
            self.channel_on = false;
        }
    }

    fn output(&self) -> i16 {
        if !self.channel_on {
            return 0;
        }

        let duty_wave: [[u8; 8]; 4] = [
            [0, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 1, 1, 1],
            [0, 1, 1, 1, 1, 1, 1, 0],
        ];

        let dac_input = duty_wave[self.duty_pattern as usize][self.duty_position as usize] * self.volume;
        let dac_output = dac_input as i16;
        return dac_output
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Ch2 {
    length: u8,
    length_timer: u8,
    length_ticking: bool,
    stop_flag: bool,
    duty_pattern: u8,
    env_initial_volume: u8,
    env_up: bool,
    env_period: u8,
    env_timer: u8,
    frequency: u16,
    duty_position: u8,
    frequency_timer: u16,
    volume: u8,
    channel_on: bool
}

impl Ch2 {
    pub fn read(&self, address: u16) -> Result<u8> {
        let data = match address {
            1 => {
                (self.duty_pattern << 6) | 0b00111111
            },
            2 => {
                (self.env_initial_volume << 4) + ((self.env_up as u8) << 3) + (self.env_period)
            },
            3 => {
                0xFF
            },
            4 => {
                ((self.stop_flag as u8) << 6) | 0b10111111
            },
            _ => 0xFF
        };

        Ok(data)
    }

    pub fn write(&mut self, address: u16, data: u8) -> Result<()> {
        match address {
            1 => {
                self.duty_pattern = (data >> 6) & 0b11;
                self.length = data & 63;
                self.length_timer = 64 - self.length;
            },
            2 => {
                self.env_initial_volume = data >> 4;
                self.env_up = (data & (1 << 3)) > 0;
                self.env_period = data & 0b111;
                self.channel_on = self.channel_on & self.dac_enable();
            },
            3 => {
                self.frequency = self.frequency - (self.frequency & 0xFF) + data as u16;
            },
            4 => {
                let freq_upper_bit = (data & 0b111) as u16;
                self.frequency = (self.frequency & 0xFF) + (freq_upper_bit << 8);
                let prev_stop_flag = self.stop_flag;
                self.stop_flag = (data & (1 << 6)) > 0;
                
                if !prev_stop_flag && self.stop_flag && self.length_ticking {
                    self.length();
                }

                if (data & (1 << 7)) > 0 {
                    self.trigger();
                }
            },
            _ => {}
        }

        Ok(())
    }

    fn dac_enable(&self) -> bool {
        return (self.env_initial_volume > 0) || self.env_up;
    }

    fn trigger(&mut self) {
        self.channel_on = self.dac_enable();
        if self.length_timer == 0 {
            self.length_timer = 64;
            if self.stop_flag && self.length_ticking {
                self.length();
            }
        }
        self.frequency_timer = (2048 - self.frequency) * 4;
        self.env_timer = self.env_period;
        self.volume = self.env_initial_volume;
    }

    fn frequency_tick(&mut self) {
        self.frequency_timer = self.frequency_timer.wrapping_sub(1);

        if self.frequency_timer == 0 {
            self.frequency_timer = (2048 - self.frequency) * 4;
            self.duty_position += 1;
            self.duty_position %= 8;
        }
    }

    fn envelope(&mut self) {
        if self.env_period != 0 {
            if self.env_timer > 0 {
                self.env_timer = self.env_timer.wrapping_sub(1);
            }

            if self.env_timer == 0 {
                self.env_timer = self.env_period;

                // 音を上げる
                if self.env_up {
                    if self.volume < 15 {
                        self.volume = self.volume.wrapping_add(1);
                    }
                }
                else {
                    if self.volume > 0 {
                        self.volume = self.volume.wrapping_sub(1);
                    }
                }
            }
        }
    }

    fn length(&mut self) {
        if !self.stop_flag || self.length_timer == 0 {
            return;
        }

        self.length_timer = self.length_timer.wrapping_sub(1);
        if self.length_timer == 0 {
            self.channel_on = false;
        }
    }

    fn output(&self) -> i16 {
        if !self.channel_on {
            return 0;
        }

        let duty_wave: [[u8; 8]; 4] = [
            [0, 0, 0, 0, 0, 0, 0, 1],
            [0, 0, 0, 0, 0, 0, 1, 1],
            [0, 0, 0, 0, 1, 1, 1, 1],
            [1, 1, 1, 1, 1, 1, 0, 0],
        ];

        let dac_input = duty_wave[self.duty_pattern as usize][self.duty_position as usize] * self.volume;
        let dac_output = dac_input as i16;
        return dac_output
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Ch3 {
    channel_on: bool,
    length: u8,
    length_timer: u16,
    length_ticking: bool,
    volume: u8,
    frequency: u16,
    frequency_timer: u16,
    position: u8,
    wave_pattern_ram: [u8; 16],
    enable: bool,
    stop_flag: bool,
}

impl Ch3 {
    pub fn read(&self, address: u16) -> Result<u8> {
        let data = match address {
            0 => {
                ((self.enable as u8) << 7) | 0b01111111
            },
            1 => {
                0xFF
            },
            2 => {
                (self.volume << 5) | 0b10011111
            },
            3 => {
                0xFF
            },
            4 => {
                ((self.stop_flag as u8) << 6) | 0b10111111
            },
            0x30..=0x3F => {
                self.wave_pattern_ram[address as usize - 0x30]
            },
            _ => 0xFF
        };

        Ok(data)
    }

    pub fn write(&mut self, address: u16, data: u8) -> Result<()> {
        match address {
            0 => {
                self.enable = (data & (1 << 7)) > 0;
                self.channel_on = self.channel_on & self.enable;
            },
            1 => {
                self.length = data;
                self.length_timer = 256 - self.length as u16;
            },
            2 => {
                self.volume = (data >> 5) & 0b11;
            },
            3 => {
                self.frequency = self.frequency - (self.frequency & 0xFF) + data as u16;
            },
            4 => {
                let freq_upper_bit = (data & 0b111) as u16;
                self.frequency = (self.frequency & 0xFF) + (freq_upper_bit << 8);
                let prev_stop_flag = self.stop_flag;
                self.stop_flag = (data & (1 << 6)) > 0;

                if !prev_stop_flag && self.stop_flag && self.length_ticking {
                    self.length();
                }

                if (data & (1 << 7)) > 0 {
                    self.trigger();
                }
            },
            0x30..=0x3F => {
                self.wave_pattern_ram[address as usize - 0x30] = data;
            }
            _ => {}
        }

        Ok(())
    }

    fn dac_enable(&self) -> bool {
        return self.enable
    }

    fn trigger(&mut self) {
        self.channel_on = self.dac_enable();
        if self.length_timer == 0 {
            self.length_timer = 256;
            if self.stop_flag && self.length_ticking {
                self.length();
            }
        }
        self.frequency_timer = (2048 - self.frequency) * 2;
        self.position = 0;
    }

    fn frequency_tick(&mut self) {
        self.frequency_timer = self.frequency_timer.wrapping_sub(1);

        if self.frequency_timer == 0 {
            self.frequency_timer = (2048 - self.frequency) * 2;
            self.position += 1;
            self.position %= 32;
        }
    }

    fn length(&mut self) {
        if !self.stop_flag || self.length_timer == 0 {
            return;
        }

        self.length_timer = self.length_timer.wrapping_sub(1);
        if self.length_timer == 0 {
            self.channel_on = false;
        }
    }

    fn output(&self) -> i16 {
        let raw_sample: i16 = if self.position % 2 == 0 {
            (self.wave_pattern_ram[self.position as usize / 2] >> 4) as i16
        }
        else {
            (self.wave_pattern_ram[self.position as usize / 2] % 0x0F) as i16
        };

        let dac_output: i16 = if self.volume == 0 || !self.channel_on {
            0
        }
        else {
            ((raw_sample << 2) >> (self.volume)) as i16
        };

        dac_output
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Ch4 {
    length: u8,
    length_timer: u8,
    length_ticking: bool,
    stop_flag: bool,
    env_initial_volume: u8,
    env_up: bool,
    env_period: u8,
    env_timer: u8,
    frequency_timer: u16,
    divisor: u8,
    divisor_code: u8,
    shift_amount: u8,
    lfsr: u16,
    counter_width: bool,
    volume: u8,
    channel_on: bool
}

impl Ch4 {
    pub fn read(&self, address: u16) -> Result<u8> {
        let data = match address {
            1 => {
                0xFF
            },
            2 => {
                (self.env_initial_volume << 4) + ((self.env_up as u8) << 3) + (self.env_period)
            },
            3 => {
                (self.shift_amount << 4) + ((self.counter_width as u8) << 3) + self.divisor_code
            },
            4 => {
                ((self.stop_flag as u8) << 6) | 0b10111111
            },
            _ => 0xFF
        };

        Ok(data)
    }

    pub fn write(&mut self, address: u16, data: u8) -> Result<()> {
        match address {
            1 => {
                self.length = data & 63;
                self.length_timer = 64 - self.length;
            },
            2 => {
                self.env_initial_volume = data >> 4;
                self.env_up = (data & (1 << 3)) > 0;
                self.env_period = data & 0b111;
                self.channel_on = self.channel_on & self.dac_enable();
            },
            3 => {
                self.shift_amount = data >> 4;
                self.counter_width = (data & (1 << 3)) > 0;
                self.divisor_code = data & 0b111;

                if self.divisor_code == 0 {
                    self.divisor = 8;
                }
                else {
                    self.divisor = 16 * self.divisor_code;
                }
            },
            4 => {
                let prev_stop_flag = self.stop_flag;
                self.stop_flag = (data & (1 << 6)) > 0;

                if !prev_stop_flag && self.stop_flag && self.length_ticking {
                    self.length();
                }
                
                if (data & (1 << 7)) > 0 {
                    self.trigger();
                }
            },
            _ => {}
        }

        Ok(())
    }

    fn dac_enable(&self) -> bool {
        return (self.env_initial_volume > 0) || self.env_up;
    }

    fn trigger(&mut self) {
        self.channel_on = self.dac_enable();
        if self.length_timer == 0 {
            self.length_timer = 64;
            if self.stop_flag && self.length_ticking {
                self.length();
            }
        }
        self.frequency_timer = (self.divisor as u16) << (self.shift_amount as u16);
        self.env_timer = self.env_period;
        self.volume = self.env_initial_volume;
        self.lfsr = 0x7FFF;
    }

    fn frequency_tick(&mut self) {
        self.frequency_timer = self.frequency_timer.wrapping_sub(1);

        if self.frequency_timer == 0 {
            // divisor codeは事前に変換しておく
            self.frequency_timer = (self.divisor as u16) << (self.shift_amount as u16);
            let xor_result = ((self.lfsr & 0b01) > 0) ^ ((self.lfsr & 0b10) > 0);

            self.lfsr = if self.counter_width {
                ((self.lfsr >> 1) & !(1 << 6)) | ((xor_result as u16) << 6)
            }
            else {
                (self.lfsr >> 1) | ((xor_result as u16) << 14)
            };
        }
    }

    fn envelope(&mut self) {
        if self.env_period != 0 {
            if self.env_timer > 0 {
                self.env_timer = self.env_timer.wrapping_sub(1);
            }

            if self.env_timer == 0 {
                self.env_timer = self.env_period;

                // 音を上げる
                if self.env_up {
                    if self.volume < 15 {
                        self.volume = self.volume.wrapping_add(1);
                    }
                }
                else {
                    if self.volume > 0 {
                        self.volume = self.volume.wrapping_sub(1);
                    }
                }
            }
        }
    }

    fn length(&mut self) {
        if !self.stop_flag || self.length_timer == 0 {
            return;
        }

        self.length_timer = self.length_timer.wrapping_sub(1);
        if self.length_timer == 0 {
            self.channel_on = false;
        }
    }

    fn output(&self) -> i16 {
        if !self.channel_on {
            return 0;
        }

        let dac_input = ((self.lfsr & 1) ^ 1) as i16;
        let dac_output = match dac_input {
            0 => 0,
            _ => self.volume as i16
        };
        return dac_output
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SoundControl {
    left_volume: u8,
    right_volume: u8,
    select_output: u8,
    vin_left: bool,
    vin_right: bool,
    sound_on: bool
}

pub struct Sound {
    ch1: Ch1,
    ch2: Ch2,
    ch3: Ch3,
    ch4: Ch4,
    current_cycle: usize,
    frame_step: u8,
    prev_bit: bool,
    sound_control: SoundControl,
    sound_buffer: ring_buffer::Bounded<Vec<Stereo<f32>>>,
    sample_rate: usize,
}

impl Sound {
    pub fn new(sample_rate: usize, buffer_size: usize) -> Result<Self> {
        let sound = Self { 
            ch1: Default::default(), 
            ch2: Default::default(), 
            ch3: Default::default(), 
            ch4: Default::default(), 
            frame_step: 7,
            current_cycle: Default::default(),
            prev_bit: Default::default(),
            sound_control: Default::default(), 
            sound_buffer: ring_buffer::Bounded::from(vec![[0.0, 0.0]; buffer_size]),
            sample_rate,
        };

        Ok(sound)
    }

    pub fn read(&self, address: u16) -> Result<u8> {
        let data = match address {
            0xFF10..=0xFF14 => self.ch1.read(address - 0xFF10),
            0xFF15..=0xFF19 => self.ch2.read(address - 0xFF15),
            0xFF1A..=0xFF1E => self.ch3.read(address - 0xFF1A),
            0xFF1F..=0xFF23 => self.ch4.read(address - 0xFF1F),
            0xFF24 => {
                let ret = ((self.sound_control.vin_left as u8) << 7) | 
                            (self.sound_control.left_volume << 4) |
                            ((self.sound_control.vin_right as u8) << 3) | 
                            (self.sound_control.right_volume);
                Ok(ret)
            },
            0xFF25 => {
                Ok(self.sound_control.select_output)
            },
            0xFF26 => {
                let mut ret = ((self.sound_control.sound_on as u8) << 7) + 
                    ((self.ch4.channel_on as u8) << 3) + 
                    ((self.ch3.channel_on as u8) << 2) +
                    ((self.ch2.channel_on as u8) << 1) +
                    ((self.ch1.channel_on as u8) << 0);

                ret |= 0b01110000;
                Ok(ret)
            }
            0xFF30..=0xFF3F => self.ch3.read(address - 0xFF00),
            _ => Ok(0xFF)
        };

        data
    }

    pub fn write(&mut self, address: u16, data: u8) -> Result<()> {
        if self.sound_control.sound_on {
            match address {
                0xFF10..=0xFF14 => self.ch1.write(address - 0xFF10, data),
                0xFF15..=0xFF19 => self.ch2.write(address - 0xFF15, data),
                0xFF1A..=0xFF1E => self.ch3.write(address - 0xFF1A, data),
                0xFF1F..=0xFF23 => self.ch4.write(address - 0xFF1F, data),
                0xFF24 => {
                    self.sound_control.vin_left = (data & (1 << 7)) > 0;
                    self.sound_control.vin_right = (data & (1 << 3)) > 0;
                    self.sound_control.left_volume = ((data & 0b01110000) >> 4) & 0b111;
                    self.sound_control.right_volume = data & 0b111;
                    Ok(())
                },
                0xFF25 => {
                    self.sound_control.select_output = data;
                    Ok(())
                },
                0xFF26 => {
                    let prev_sound_on = self.sound_control.sound_on;
                    self.sound_control.sound_on = (data & (1 << 7)) > 0;
                    if prev_sound_on && !self.sound_control.sound_on {
                        self.reset();
                    }
                    else if !prev_sound_on && self.sound_control.sound_on {
                        self.frame_step = 7;
                    }
                    Ok(())
                }
                0xFF30..=0xFF3F => self.ch3.write(address - 0xFF00, data),
                _ => Ok(())
            }
        }
        else {
            match address {
                0xFF26 => {
                    self.sound_control.sound_on = (data & (1 << 7)) > 0;
                    if !self.sound_control.sound_on {
                        self.reset();
                    }
                    Ok(())
                }
                0xFF30..=0xFF3F => self.ch3.write(address - 0xFF00, data),
                _ => Ok(())
            }
        }
    }

    fn reset(&mut self) {
        let mut ch3 = Ch3::default();
        ch3.wave_pattern_ram = self.ch3.wave_pattern_ram;
        let sound = Sound { 
            ch1: Default::default(), 
            ch2: Default::default(), 
            ch3,
            ch4: Default::default(), 
            frame_step: 7,
            current_cycle: Default::default(),
            prev_bit: self.prev_bit,
            sound_control: Default::default(), 
            sound_buffer: self.sound_buffer.clone(),
            sample_rate: self.sample_rate,
        };

        *self = sound
    }

    pub fn tick(&mut self, div: u8) {
        if !self.sound_control.sound_on {
            return
        }

        self.current_cycle = self.current_cycle.wrapping_add(1);
        self.ch1.frequency_tick();
        self.ch2.frequency_tick();
        self.ch3.frequency_tick();
        self.ch4.frequency_tick();

        let cur_bit = (div & (1 << 4)) > 0;

        if self.prev_bit && !cur_bit{
            self.frame_step = self.frame_step.wrapping_add(1);
            self.frame_step %= 8;

            match self.frame_step {
                0 => self.length(),
                1 | 3 | 5 => {
                    self.ch1.length_ticking = false;
                    self.ch2.length_ticking = false;
                    self.ch3.length_ticking = false;
                    self.ch4.length_ticking = false;
                },
                2 | 6 => {
                    self.sweep();
                    self.length();
                },
                4 => self.length(),
                7 => {
                    self.envelope();
                    self.ch1.length_ticking = false;
                    self.ch2.length_ticking = false;
                    self.ch3.length_ticking = false;
                    self.ch4.length_ticking = false;
                },
                _ => {}
            }
        }

        self.prev_bit = cur_bit;

        let output_cycle = 4194304 / self.sample_rate;
        if self.current_cycle >= output_cycle {
            self.current_cycle -= output_cycle;
            let sample = self.mix();
            if !self.sound_buffer.is_full() {
                self.sound_buffer.push(sample);
            }
        }
    }

    fn envelope(&mut self) {
        // ch1, ch2, ch4
        self.ch1.envelope();
        self.ch2.envelope();
        self.ch4.envelope();
    }

    fn sweep(&mut self) {
        self.ch1.sweep();
    }

    fn length(&mut self) {
        self.ch1.length_ticking = true;
        self.ch2.length_ticking = true;
        self.ch3.length_ticking = true;
        self.ch4.length_ticking = true;
        self.ch1.length();
        self.ch2.length();
        self.ch3.length();
        self.ch4.length();
    }

    fn mix(&mut self) -> Stereo<f32> {
        let mut left = 0.0;
        let mut right = 0.0;

        let left_volume = (self.sound_control.left_volume as f32 / 7.0) * (1.0 / 15.0) * 0.25;
        let right_volume = (self.sound_control.right_volume as f32 / 7.0) * (1.0 / 15.0) * 0.25;

        // right
        for i in 0..4 {
            if (self.sound_control.select_output & (1 << i)) > 0 {
                match i {
                    0 => right += (self.ch1.output() as f32) * right_volume,
                    1 => right += (self.ch2.output() as f32) * right_volume,
                    2 => right += (self.ch3.output() as f32 / 4.0) * right_volume,
                    3 => right += (self.ch4.output() as f32) * right_volume,
                    _ => {}
                }
            }
        }

        // left
        for i in 4..8 {
            if (self.sound_control.select_output & (1 << i)) > 0 {
                match i {
                    4 => left += (self.ch1.output() as f32) * left_volume,
                    5 => left += (self.ch2.output() as f32) * left_volume,
                    6 => left += (self.ch3.output() as f32 / 4.0) * left_volume,
                    7 => left += (self.ch4.output() as f32) * left_volume,
                    _ => {}
                }
            }
        }

        return [left, right]
    }

    pub fn get_sound_buffer(&mut self) -> &mut ring_buffer::Bounded<Vec<Stereo<f32>>> {
        return &mut self.sound_buffer
    }
}