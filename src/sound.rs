use anyhow::Result;
use cpal;
use dasp::{Signal, Sample, self as signal, ring_buffer, frame::Stereo};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

#[derive(Clone, Copy, Debug, Default)]
pub struct Ch1 {
    sweep: u8,
    sound_length: u8,
    wave_pattern: u8,
    volume_envelope: u8,
    frequency_low: u8,
    frequency_high: u8
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Ch2 {
    sound_length: u8,
    wave_pattern: u8,
    volume_envelope: u8,
    frequency_low: u8,
    frequency_high: u8
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Ch3 {
    sound_on: bool,
    sound_length: u8,
    select_output_level: u8,
    frequency_low: u8,
    frequency_high: u8,
    wave_pattern_ram: [u8; 16]
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Ch4 {
    sound_length: u8,
    volume_envelope: u8,
    polynomical_counter: u8,
    select_counter_consecutive: u8
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SoundControl {
    channel_control: u8,
    select_sound_output_terminal: u8,
    sound_on: u8
}

pub struct Sound {
    ch1: Ch1,
    ch2: Ch2,
    ch3: Ch3,
    ch4: Ch4,
    frame_sequence: u16,
    current_cycle: u16,
    sound_control: SoundControl,
    sound_buffer: ring_buffer::Bounded<Vec<Stereo<f32>>>,
    device: cpal::Device,
    config: cpal::StreamConfig
}

impl Sound {
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host.default_output_device().expect("failed to find a default output device");
        let config = device.default_output_config()?;

        let sound = Self { 
            ch1: Default::default(), 
            ch2: Default::default(), 
            ch3: Default::default(), 
            ch4: Default::default(), 
            frame_sequence: Default::default(),
            current_cycle: Default::default(),
            sound_control: Default::default(), 
            sound_buffer: ring_buffer::Bounded::from(vec![[0.0, 0.0]; 44100]),
            device,
            config: config.into()
        };

        Ok(sound)
    }

    pub fn tick(&mut self) {

    }

    pub fn get_sound_buffer(&mut self) -> &mut ring_buffer::Bounded<Vec<Stereo<f32>>> {
        return &mut self.sound_buffer
    }
}