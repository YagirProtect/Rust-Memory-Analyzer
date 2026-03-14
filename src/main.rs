use crate::app::App;

mod app;
mod classes;
mod screens;
mod utils;

fn main() {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native("Memory Viewer", native_options, Box::new(|cc| Ok(Box::new(App::new(cc)))));
}
