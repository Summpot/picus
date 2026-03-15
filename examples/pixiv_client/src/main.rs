#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;

fn main() -> std::result::Result<(), picus::xilem::winit::error::EventLoopError> {
    app::run()
}
