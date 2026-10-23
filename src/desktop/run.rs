use eframe::egui;

use crate::{App, store::RecordRepository};

#[derive(Debug, thiserror::Error)]
pub enum DesktopError {
    #[error(transparent)]
    Egui(#[from] eframe::Error),
}

#[derive(Debug, Default)]
struct AppState {
    name: String,
    age: u32,
}

impl eframe::App for AppState {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let _ = egui::CentralPanel::default().show(ui, |ui| {
            let _ = ui.heading("Workaholick");
            let _ = ui.label("Frontend running");
        });
    }
}

pub(crate) fn run<S: RecordRepository>(app: &mut App<S>) -> Result<(), DesktopError> {
    env_logger::init();

    let options =
        eframe::NativeOptions { viewport: egui::ViewportBuilder::default(), ..Default::default() };
    eframe::run_native("Workaholick", options, Box::new(|_| Ok(Box::<AppState>::default())))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use log::{debug, error};

    #[test]
    fn print_test() {
        env_logger::init();
        debug!("this is a debug {}", "message");
        error!("this is printed by default");

        println!("test");
    }
}
