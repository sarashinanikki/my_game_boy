use std::fs::File;
use cpal::traits::{HostTrait, DeviceTrait, StreamTrait};
use dotenvy::dotenv;
use joypad::Button;
use std::env;
use std::io::BufReader;
use std::time::{Duration, Instant};
use std::thread::sleep;

use winit::event::{Event, WindowEvent, KeyboardInput, ElementState, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit::dpi::LogicalSize;
use pixels::{Pixels, SurfaceTexture};
use cpal::{self, StreamError, SampleFormat, OutputCallbackInfo};
use dasp::{Frame, Sample, Signal};

mod rom;
mod mbc;
mod bus;
mod cpu;
mod ppu;
mod joypad;
mod timer;
mod sound;

fn main() {
    dotenv().ok();
    let args: Vec<String> = env::args().collect();
    let rom_name = &args[1];
    let base_path = env::var("BASE_PATH").expect("BASE_PATH must be set!");

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("My Game Boy")
        .with_inner_size(LogicalSize::new(160, 144))
        .with_min_inner_size(LogicalSize::new(160, 144))
        .build(&event_loop)
        .unwrap();

    let window_size = window.inner_size();
    let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
    let mut pixels = Pixels::new(160, 144, surface_texture).unwrap();

    let file_path = base_path + rom_name;
    let mut reader = BufReader::new(File::open(file_path).unwrap());
    let bus = bus::Bus::new(&mut reader);
    let mut cpu = cpu::Cpu::new(bus);
    cpu.reset();
    cpu.bus.mbc.read_save_file().unwrap();

    // 音声
    let host = cpal::default_host();
    let device = host.default_output_device().expect("failed to find a default output device");
    let config = device.default_output_config().unwrap();
    let err_fn = |err: StreamError| eprintln!("an error occured in sound stream: {}", err);
    let channels = config.channels();

    let data_callback_f32 = |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        write_audio_data(data, channels as usize, &mut signal);
    };

    let data_callback_i16 = |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
        write_audio_data(data, channels as usize, &mut signal);
    };

    let data_callback_u16 = |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
        write_audio_data(data, channels as usize, &mut signal);
    };

    let stream = match config.sample_format() {
        SampleFormat::F32 => {
            device.build_output_stream(
                &config.into(), 
                data_callback_f32,
                err_fn
            ).unwrap()
        },
        SampleFormat::I16 => {
            device.build_output_stream(
                &config.into(), 
                data_callback_i16,
                err_fn
            ).unwrap()
        },
        SampleFormat::U16 => {
            device.build_output_stream(
                &config.into(), 
                data_callback_u16,
                err_fn
            ).unwrap()
        }
    }; 

    stream.play().unwrap();

    // 画面描画
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    cpu.bus.mbc.write_save_file().unwrap();
                    *control_flow = ControlFlow::Exit
                },
                WindowEvent::KeyboardInput { 
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(virtual_code),
                            state: button_state,
                            ..
                        },
                    ..
                } => match virtual_code {
                    VirtualKeyCode::E => {
                        match button_state {
                            ElementState::Pressed => cpu.bus.joypad.press(Button::Up),
                            ElementState::Released => cpu.bus.joypad.release(Button::Up)
                        }
                    },
                    VirtualKeyCode::D => {
                        match button_state {
                            ElementState::Pressed => cpu.bus.joypad.press(Button::Down),
                            ElementState::Released => cpu.bus.joypad.release(Button::Down)
                        }
                    }
                    VirtualKeyCode::S => {
                        match button_state {
                            ElementState::Pressed => cpu.bus.joypad.press(Button::Left),
                            ElementState::Released => cpu.bus.joypad.release(Button::Left)
                        }
                    }
                    VirtualKeyCode::F => {
                        match button_state {
                            ElementState::Pressed => cpu.bus.joypad.press(Button::Right),
                            ElementState::Released => cpu.bus.joypad.release(Button::Right)
                        }
                    },
                    VirtualKeyCode::J => {
                        match button_state {
                            ElementState::Pressed => cpu.bus.joypad.press(Button::B),
                            ElementState::Released => cpu.bus.joypad.release(Button::B)
                        }
                    },
                    VirtualKeyCode::K => {
                        match button_state {
                            ElementState::Pressed => cpu.bus.joypad.press(Button::A),
                            ElementState::Released => cpu.bus.joypad.release(Button::A)
                        }
                    },
                    VirtualKeyCode::G => {
                        match button_state {
                            ElementState::Pressed => cpu.bus.joypad.press(Button::Select),
                            ElementState::Released => cpu.bus.joypad.release(Button::Select)
                        }
                    },
                    VirtualKeyCode::H => {
                        match button_state {
                            ElementState::Pressed => cpu.bus.joypad.press(Button::Start),
                            ElementState::Released => cpu.bus.joypad.release(Button::Start)
                        }
                    },
                    VirtualKeyCode::N => {
                        match button_state {
                            ElementState::Pressed => {
                                cpu.debug_flag ^= true;
                            },
                            ElementState::Released => {}
                        }
                    },
                    VirtualKeyCode::M => {
                        match button_state {
                            ElementState::Pressed => {
                                cpu.step_flag ^= true;
                            },
                            ElementState::Released => {}
                        }
                    }
                    _ => {}
                },
                WindowEvent::Resized(size) => {
                    pixels.resize_surface(size.width, size.height);
                },
                _ => {}
            },
            Event::MainEventsCleared => {
                let start = Instant::now();
                cpu.run().unwrap();
                let duration = start.elapsed().as_micros();
                let frame_microsec: u128 = 1_000_000 / 60;
                
                if duration < frame_microsec {
                    let wait_time: u128 = frame_microsec - duration;
                    sleep(Duration::from_micros(wait_time as u64));
                }
        
                window.request_redraw();                
            }
            Event::RedrawRequested(_) => {
                cpu.render(pixels.get_frame());
                if pixels.render().is_err() {
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                *control_flow = ControlFlow::Poll;
            },
            _ => {}
        }
    })
}

fn write_audio_data<T> (
    output: &mut[T],
    channels: usize,
    signal: &mut dyn Signal<Frame = f32>
) where
    T: cpal::Sample,
{
    for frame in output.chunks_mut(channels) {
        let value: T = cpal::Sample::from::<f32>(&signal.next());
        for sample in frame.iter_mut() {
            *sample = value;
        }
    }
}