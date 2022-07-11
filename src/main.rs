use std::fs::File;
use std::io::BufReader;
use std::time::{Duration, Instant};
use std::thread::sleep;

use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit::dpi::LogicalSize;
use pixels::{Pixels, SurfaceTexture};
use winit_input_helper::WinitInputHelper;

mod rom;
mod mbc;
mod bus;
mod cpu;
mod ppu;

fn main() {
    let mut input = WinitInputHelper::new();
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

    let mut reader = BufReader::new(File::open("/home/sarashin/Hobby/my_game_boy/rom/hello-world.gb").unwrap());
    let rom = rom::Rom::new(&mut reader).unwrap();
    let mbc = Box::new(mbc::NoMbc{mbc_type: 0, rom});
    let ppu = ppu::Ppu::new();
    let bus = bus::Bus::new(mbc, ppu);
    let mut cpu = cpu::Cpu::new(bus);
    
    event_loop.run(move |event, _, control_flow| {
        let start = Instant::now();

        if input.update(&event) {
            if input.key_released(VirtualKeyCode::Escape) || input.quit() {
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        cpu.run().unwrap();

        if let Event::RedrawRequested(_) = event {
            cpu.render(pixels.get_frame());
            if pixels.render().is_err() {
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        let duration = start.elapsed().as_micros();
        let frame_microsec: u128 = 1_000_000 / 60;
        
        if duration < frame_microsec {
            let wait_time: u128 = frame_microsec - duration;
            sleep(Duration::from_micros(wait_time as u64));
        }

        window.request_redraw();
    })
}