#![deny(clippy::all)]
#![forbid(unsafe_code)]

use crate::shapes::Shapes;
use log::error;
use pixels::{Pixels, SurfaceTexture};
use std::time::Instant;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

mod shapes;

#[cfg_attr(
    target_os = "android",
    ndk_glue::main(backtrace = "on", logger(level = "debug", tag = "raqote-winit"))
)]
pub fn main() {
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let mut window = None;
    let mut pixels = None;
    let mut shapes = None;
    let mut now = Instant::now();

    event_loop.run(move |event, event_loop, control_flow| {
        #[cfg(not(target_os = "android"))]
        let initialize = pixels.is_none() || shapes.is_none();
        #[cfg(target_os = "android")]
        let initialize = event == Event::Resumed;

        if initialize {
            let window2 = WindowBuilder::new()
                .with_title("Hello Raqote")
                .build(&event_loop)
                .unwrap();
            let window_size = window2.inner_size();
            let surface_texture =
                SurfaceTexture::new(window_size.width, window_size.height, &window2);
            let pixels2 = Pixels::new(window_size.width, window_size.height, surface_texture)
                .expect("failed to initialize pixels");
            let shapes2 = Shapes::new(window_size.width, window_size.height);

            window = Some(window2);
            pixels = Some(pixels2);
            shapes = Some(shapes2);
        }
        if Event::Suspended == event {
            window = None;
            pixels = None;
            shapes = None;
        }

        if let (Some(window), Some(pixels), Some(shapes)) = (window.as_mut(), pixels.as_mut(), shapes.as_mut()) {
            // Draw the current frame
            if let Event::RedrawRequested(_) = event {
                for (dst, &src) in pixels
                    .get_frame()
                    .chunks_exact_mut(4)
                    .zip(shapes.get_frame().iter())
                {
                    dst[0] = (src >> 16) as u8;
                    dst[1] = (src >> 8) as u8;
                    dst[2] = src as u8;
                    dst[3] = (src >> 24) as u8;
                }

                if pixels
                    .render()
                    .map_err(|e| error!("pixels.render() failed: {}", e))
                    .is_err()
                {
                    *control_flow = ControlFlow::Exit;
                    return;
                }
            }

            // Handle input events
            if input.update(&event) {
                // Close events
                if input.key_pressed(VirtualKeyCode::Escape) || input.quit() {
                    *control_flow = ControlFlow::Exit;
                    return;
                }

                // Resize the window
                if let Some(size) = input.window_resized() {
                    pixels.resize_surface(size.width, size.height);
                }

                // Update internal state and request a redraw
                shapes.draw(now.elapsed().as_secs_f32());
                window.request_redraw();

                now = Instant::now();
            }
        }
    });
}
