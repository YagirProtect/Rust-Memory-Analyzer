use eframe::egui;
use sysinfo::System;
use crate::classes::c_console::Console;
use crate::classes::c_console_row::ConsoleRow;
use crate::classes::c_opened_process::OpenedProcess;
use crate::classes::c_processes::Processes;
use crate::classes::c_select_process_window::SelectProcessWindow;
use crate::classes::e_message_type::EMessageType;

#[derive(Default)]
pub enum AppState{
    #[default]
    ProcessSelection
}
pub struct App{
    pub app_state: AppState,
    pub system: System,
    pub processes: Processes,
    pub console: Console,
    pub opened_process: Option<OpenedProcess>,
    pub select_process_window: SelectProcessWindow,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let _ = cc;
        Self::default()
    }
}

impl Default for App{
    fn default() -> Self {
        let mut system = System::new_all();
        system.refresh_all();



        Self{
            app_state: AppState::ProcessSelection,
            system: system,
            processes: Processes::default(),
            console: Console::default(),
            opened_process: None,
            select_process_window: SelectProcessWindow::default(),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let _ = frame;

        if let Some(process) = &self.opened_process {
            let has_frozen = process.watched_rows.iter().any(|row| row.is_frozen);
            let has_pending_verify = process
                .watched_rows
                .iter()
                .any(|row| row.verify_after_at.is_some());

            if has_frozen || has_pending_verify {
                ctx.request_repaint_after(std::time::Duration::from_millis(16));
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.app_state {
                AppState::ProcessSelection => {
                    self.draw(ctx, ui);
                }
            }
        });
    }
}
