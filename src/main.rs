use std::fs::File;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::{env, thread};
use std::io::BufReader;
use std::time::{Duration, Instant};
use std::future::Future;

use cpal::traits::{HostTrait, DeviceTrait, StreamTrait};
use cpal::{self, StreamError, SampleFormat, OutputCallbackInfo, Stream};
use dasp::frame::Stereo;
use dasp::ring_buffer::Bounded;
use dasp::{Frame, Sample, Signal};

use dotenvy::dotenv;

use web_sys::console;
use winit::event::{Event, WindowEvent, KeyboardInput, ElementState, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit::dpi::LogicalSize;
use serde::Serialize;
use pixels::{Pixels, SurfaceTexture};

mod rom;
mod mbc;
mod bus;
mod cpu;
mod ppu;
mod joypad;
mod timer;
mod sound;

use joypad::{Button, KeyConfig};

fn main() {
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        console_log::init_with_level(log::Level::Trace).expect("error initializing logger");
        wasm_bindgen_futures::spawn_local(web_run());
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        run();
    }
}

#[cfg(target_arch = "wasm32")]
async fn web_run() {
    use log::{Level, info};
    use wasm_bindgen::prelude::*;

    use std::io::Cursor;
    use gloo::storage::LocalStorage;
    use gloo_storage::Storage;
    use gloo_storage::errors::StorageError;
    use base64::decode;

    use serde::Serializer;

    // LocalStorageからROMデータを読み出す
    let local_storage = web_sys::window().unwrap().local_storage().unwrap().unwrap();
    let buf_vec = match local_storage.get_item("rom").unwrap() {
        Some(res) => {
            decode(res).unwrap()
        },
        None => {
            log::trace!("No ROM bin");
            return;
        }
    };

    // LocalStorageからキーコンフィグ情報を読み出す
    let key_config: KeyConfig = match local_storage.get_item("keyInfo").unwrap() {
        Some(res) => {
            serde_json::from_str(&res).unwrap()
        },
        None => {
            KeyConfig {
                RIGHT: "S".to_string(),
                LEFT: "F".to_string(),
                UP: "E".to_string(),
                DOWN: "D".to_string(),
                A: "K".to_string(),
                B: "J".to_string(),
                SELECT: "Space".to_string(),
                START: "Return".to_string()
            }
        }
    };

    let buf = &buf_vec[..];
    let mut reader = Cursor::new(buf);

    // 音声デバイス作成
    let host = cpal::default_host();
    let device = host.default_output_device().expect("failed to find a default output device");
    let config = device.default_output_config().unwrap();
    let sample_rate = config.sample_rate().0 as usize;

    // cpu作成
    let bus = bus::Bus::new(&mut reader, sample_rate, 4000);
    let cpu = Arc::new(Mutex::new(cpu::Cpu::new(bus)));

    // GUI生成
    let event_loop = EventLoop::new();
    let window_ = WindowBuilder::new()
        .with_title("My Game Boy")
        .with_inner_size(LogicalSize::new(160, 144))
        .with_min_inner_size(LogicalSize::new(160, 144))
        .build(&event_loop)
        .unwrap();
    
    let window = Rc::new(window_);

    // Canvasをhtmlのbodyにappendする
    {
        use wasm_bindgen::JsCast;
        use winit::platform::web::WindowExtWebSys;

        // Retrieve current width and height dimensions of browser client window
        let get_window_size = || {
            let client_window = web_sys::window().unwrap();
            LogicalSize::new(
                client_window.inner_width().unwrap().as_f64().unwrap(),
                client_window.inner_height().unwrap().as_f64().unwrap(),
            )
        };

        let window = Rc::clone(&window);

        // Initialize winit window with current dimensions of browser client
        window.set_inner_size(get_window_size());

        let client_window = web_sys::window().unwrap();

        // Attach winit canvas to body element
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| doc.body())
            .and_then(|body| {
                body.append_child(&web_sys::Element::from(window.canvas()))
                    .ok()
            })
            .expect("couldn't append canvas to document body");

        // Listen for resize event on browser client. Adjust winit window dimensions
        // on event trigger
        let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |_e: web_sys::Event| {
            let size = get_window_size();
            window.set_inner_size(size)
        }) as Box<dyn FnMut(_)>);
        client_window
            .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    // 描画バックエンド(WebGPU)生成
    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, window.as_ref());
        Pixels::new_async(160, 144, surface_texture)
            .await
            .expect("Pixels error")
    };

    // 音声設定
    let cpu_sound = cpu.clone();
    let channels = config.channels() as usize;
    let err_fn = |err: StreamError| eprintln!("an error occured in sound stream: {}", err);
    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_output_stream(&config.into(), move |data: &mut [f32], _: &cpal::OutputCallbackInfo| { write_data(data, channels, &cpu_sound) }, err_fn),
        SampleFormat::I16 => device.build_output_stream(&config.into(), move |data: &mut [i16], _: &cpal::OutputCallbackInfo| { write_data(data, channels, &cpu_sound) }, err_fn),
        SampleFormat::U16 => device.build_output_stream(&config.into(), move |data: &mut [u16], _: &cpal::OutputCallbackInfo| { write_data(data, channels, &cpu_sound) }, err_fn)
    }.unwrap();
    console::log_1(&JsValue::from_f64(channels as f64));

    // cpuの実行
    let cpu_a = cpu.clone();
    wasm_bindgen_futures::spawn_local(run_cpu(cpu_a));

    // 音声再生の開始
    stream.play().unwrap();

    let mut current_time = instant::Instant::now();
    // 画面描画ループ
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    // cpu.lock().unwrap().bus.mbc.write_save_file().unwrap();
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
                } => {
                    let key_code_input: String = virtual_code.serialize(serde_json::value::Serializer).unwrap().to_string();
                    let input_len = key_code_input.len();

                    if let Some(button) = key_config.find_key(&key_code_input[1..input_len-1]) {
                        match button {
                            Button::A => {
                                match button_state {
                                    ElementState::Pressed => cpu.lock().unwrap().bus.joypad.press(Button::A),
                                    ElementState::Released => cpu.lock().unwrap().bus.joypad.release(Button::A)
                                }
                            },
                            Button::B => {
                                match button_state {
                                    ElementState::Pressed => cpu.lock().unwrap().bus.joypad.press(Button::B),
                                    ElementState::Released => cpu.lock().unwrap().bus.joypad.release(Button::B)
                                }
                            },
                            Button::Down => {
                                match button_state {
                                    ElementState::Pressed => cpu.lock().unwrap().bus.joypad.press(Button::Down),
                                    ElementState::Released => cpu.lock().unwrap().bus.joypad.release(Button::Down)
                                }
                            },
                            Button::Left => {
                                match button_state {
                                    ElementState::Pressed => cpu.lock().unwrap().bus.joypad.press(Button::Left),
                                    ElementState::Released => cpu.lock().unwrap().bus.joypad.release(Button::Left)
                                }
                            },
                            Button::Right => {
                                match button_state {
                                    ElementState::Pressed => cpu.lock().unwrap().bus.joypad.press(Button::Right),
                                    ElementState::Released => cpu.lock().unwrap().bus.joypad.release(Button::Right)
                                }
                            },
                            Button::Select => {
                                match button_state {
                                    ElementState::Pressed => cpu.lock().unwrap().bus.joypad.press(Button::Select),
                                    ElementState::Released => cpu.lock().unwrap().bus.joypad.release(Button::Select)
                                }
                            },
                            Button::Start => {
                                match button_state {
                                    ElementState::Pressed => cpu.lock().unwrap().bus.joypad.press(Button::Start),
                                    ElementState::Released => cpu.lock().unwrap().bus.joypad.release(Button::Start)
                                }
                            },
                            Button::Up => {
                                match button_state {
                                    ElementState::Pressed => cpu.lock().unwrap().bus.joypad.press(Button::Up),
                                    ElementState::Released => cpu.lock().unwrap().bus.joypad.release(Button::Up)
                                }
                            }
                        }
                    }
                },
                WindowEvent::Resized(size) => {
                    pixels.resize_surface(size.width, size.height);
                },
                _ => {}
            },
            Event::MainEventsCleared => {
                let duration = current_time.elapsed().as_micros();
                let frame_microsec: u128 = 1_000_000 / 60;
                if duration >= frame_microsec && cpu.lock().unwrap().sleep {
                    current_time = instant::Instant::now();
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

#[cfg(target_arch = "wasm32")]
async fn run_cpu(cpu: Arc<Mutex<cpu::Cpu>>) {
    cpu.lock().unwrap().reset();
    cpu.lock().unwrap().bus.mbc.read_save_file().unwrap();
    loop {
        let start = instant::Instant::now();
        cpu.lock().unwrap().run().unwrap();
        let duration = start.elapsed().as_micros();
        let frame_microsec: u128 = 1_000_000 / 60;
        
        if duration < frame_microsec {
            let wait_time: u128 = frame_microsec - duration;
            sleep((wait_time / 1000) as i32).await;
        }
    };
}

#[cfg(target_arch = "wasm32")]
async fn match_input_key_config(config: KeyConfig, input: VirtualKeyCode) {
    
}

#[cfg(target_arch = "wasm32")]
fn sleep(ms: i32) -> impl Future {
    let p = js_sys::Promise::new(&mut |resolve, _| {
        web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms)
            .unwrap();
    });
    wasm_bindgen_futures::JsFuture::from(p)
}

#[cfg(target_arch = "wasm32")]
fn write_data<T>(output: &mut [T], channels: usize, cpu_sound: &Arc<Mutex<cpu::Cpu>>) 
where T: cpal::Sample
{
    for frame in output.chunks_mut(channels) {
        let value: [T; 2] = match cpu_sound.lock().unwrap().bus.sound.get_sound_buffer().pop() {
            Some(res) => res.map(|e| cpal::Sample::from::<f32>(&e)),
            None => Stereo::EQUILIBRIUM.map(|e| cpal::Sample::from::<f32>(&e)),
        };

        frame.copy_from_slice(&value);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run() {
    dotenv().ok();
    let args: Vec<String> = env::args().collect();
    let rom_name = &args[1];
    let base_path = env::var("BASE_PATH").unwrap_or("".to_string());

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("My Game Boy")
        .with_inner_size(LogicalSize::new(160, 144))
        .with_min_inner_size(LogicalSize::new(160, 144))
        .build(&event_loop)
        .unwrap();
    
    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(160, 144, surface_texture).expect("Pixels error")
    };

    let file_path = base_path + rom_name;
    let mut reader = BufReader::new(File::open(file_path).unwrap());

    let host = cpal::default_host();
    let device = host.default_output_device().expect("failed to find a default output device");
    let config = device.default_output_config().unwrap();
    let sample_rate = config.sample_rate().0 as usize;

    let bus = bus::Bus::new(&mut reader, sample_rate, 2000);
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
    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_output_stream(&config.into(), move |data: &mut [f32], _: &cpal::OutputCallbackInfo| { write_data(data, channels, &cpu_sound) }, err_fn),
        SampleFormat::I16 => device.build_output_stream(&config.into(), move |data: &mut [i16], _: &cpal::OutputCallbackInfo| { write_data(data, channels, &cpu_sound) }, err_fn),
        SampleFormat::U16 => device.build_output_stream(&config.into(), move |data: &mut [u16], _: &cpal::OutputCallbackInfo| { write_data(data, channels, &cpu_sound) }, err_fn)
    }.unwrap();

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
                    VirtualKeyCode::Space => {
                        match button_state {
                            ElementState::Pressed => cpu.lock().unwrap().bus.joypad.press(Button::Select),
                            ElementState::Released => cpu.lock().unwrap().bus.joypad.release(Button::Select)
                        }
                    },
                    VirtualKeyCode::Return => {
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
                if duration >= frame_microsec && cpu.lock().unwrap().sleep {
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

#[cfg(not(target_arch = "wasm32"))]
fn write_data<T>(output: &mut [T], channels: usize, cpu_sound: &Arc<Mutex<cpu::Cpu>>) 
where T: cpal::Sample
{
    for frame in output.chunks_mut(channels) {
        let value: [T; 2] = match cpu_sound.lock().unwrap().bus.sound.get_sound_buffer().pop() {
            Some(res) => res.map(|e| cpal::Sample::from::<f32>(&e)),
            None => Stereo::EQUILIBRIUM.map(|e| cpal::Sample::from::<f32>(&e)),
        };

        frame.copy_from_slice(&value);
    }
}
