use std::fs::File;
use std::sync::{Arc, Mutex};
use cpal::traits::{HostTrait, DeviceTrait, StreamTrait};
use dasp::frame::Stereo;
use dasp::ring_buffer::Bounded;
use dotenvy::dotenv;
use joypad::Button;
use std::{env, thread};
use std::io::BufReader;
use std::time::{Duration, Instant};
use std::thread::sleep;

use winit::event::{Event, WindowEvent, KeyboardInput, ElementState, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit::dpi::LogicalSize;
use pixels::{Pixels, SurfaceTexture};
use cpal::{self, StreamError, SampleFormat, OutputCallbackInfo, Stream};
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

    let host = cpal::default_host();
    let device = host.default_output_device().expect("failed to find a default output device");
    let config = device.default_output_config().unwrap();
    let sample_rate = config.sample_rate().0 as usize;

    let bus = bus::Bus::new(&mut reader, sample_rate);
    let cpu = Arc::new(Mutex::new(cpu::Cpu::new(bus)));
    
    {
        let cpu = cpu.clone();
        cpu.lock().unwrap().reset();
        cpu.lock().unwrap().bus.mbc.read_save_file().unwrap();

        thread::spawn(move || loop {
            let start = Instant::now();
            cpu.lock().unwrap().run().unwrap();
            let duration = start.elapsed().as_micros();
            let frame_microsec: u128 = 1_000_000 / 60;
            
            if duration < frame_microsec {
                let wait_time: u128 = frame_microsec - duration;
                thread::sleep(Duration::from_micros(wait_time as u64));
            }
        });
    }

    // 音声
    let cpu_sound = cpu.clone();
    let channels = config.channels() as usize;
    let err_fn = |err: StreamError| eprintln!("an error occured in sound stream: {}", err);
    let stream = device.build_output_stream(
        &config.into(),
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            for frame in data.chunks_mut(channels) {
                let value: [f32; 2] = match cpu_sound.lock().unwrap().bus.sound.get_sound_buffer().pop() {
                    Some(res) => res.map(|e| cpal::Sample::from::<f32>(&e)),
                    None => Stereo::EQUILIBRIUM.map(|e| cpal::Sample::from::<f32>(&e)),
                };
        
                frame.copy_from_slice(&value);
            }
        },
        err_fn
    ).unwrap();

    stream.play().unwrap();

    let mut current_time = Instant::now();
    // 画面描画
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    cpu.lock().unwrap().bus.mbc.write_save_file().unwrap();
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
                            ElementState::Pressed => cpu.lock().unwrap().bus.joypad.press(Button::Up),
                            ElementState::Released => cpu.lock().unwrap().bus.joypad.release(Button::Up)
                        }
                    },
                    VirtualKeyCode::D => {
                        match button_state {
                            ElementState::Pressed => cpu.lock().unwrap().bus.joypad.press(Button::Down),
                            ElementState::Released => cpu.lock().unwrap().bus.joypad.release(Button::Down)
                        }
                    }
                    VirtualKeyCode::S => {
                        match button_state {
                            ElementState::Pressed => cpu.lock().unwrap().bus.joypad.press(Button::Left),
                            ElementState::Released => cpu.lock().unwrap().bus.joypad.release(Button::Left)
                        }
                    }
                    VirtualKeyCode::F => {
                        match button_state {
                            ElementState::Pressed => cpu.lock().unwrap().bus.joypad.press(Button::Right),
                            ElementState::Released => cpu.lock().unwrap().bus.joypad.release(Button::Right)
                        }
                    },
                    VirtualKeyCode::J => {
                        match button_state {
                            ElementState::Pressed => cpu.lock().unwrap().bus.joypad.press(Button::B),
                            ElementState::Released => cpu.lock().unwrap().bus.joypad.release(Button::B)
                        }
                    },
                    VirtualKeyCode::K => {
                        match button_state {
                            ElementState::Pressed => cpu.lock().unwrap().bus.joypad.press(Button::A),
                            ElementState::Released => cpu.lock().unwrap().bus.joypad.release(Button::A)
                        }
                    },
                    VirtualKeyCode::G => {
                        match button_state {
                            ElementState::Pressed => cpu.lock().unwrap().bus.joypad.press(Button::Select),
                            ElementState::Released => cpu.lock().unwrap().bus.joypad.release(Button::Select)
                        }
                    },
                    VirtualKeyCode::H => {
                        match button_state {
                            ElementState::Pressed => cpu.lock().unwrap().bus.joypad.press(Button::Start),
                            ElementState::Released => cpu.lock().unwrap().bus.joypad.release(Button::Start)
                        }
                    },
                    VirtualKeyCode::N => {
                        match button_state {
                            ElementState::Pressed => {
                                cpu.lock().unwrap().debug_flag ^= true;
                            },
                            ElementState::Released => {}
                        }
                    },
                    VirtualKeyCode::M => {
                        match button_state {
                            ElementState::Pressed => {
                                cpu.lock().unwrap().step_flag ^= true;
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
                let duration = current_time.elapsed().as_micros();
                let frame_microsec: u128 = 1_000_000 / 60;
                if duration >= frame_microsec {
                    current_time = Instant::now();
                    window.request_redraw();    
                }
            }
            Event::RedrawRequested(_) => {
                cpu.lock().unwrap().render(pixels.get_frame());
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
