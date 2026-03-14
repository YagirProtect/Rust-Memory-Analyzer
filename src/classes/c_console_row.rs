use chrono::Local;
use eframe::egui;
use eframe::egui::Color32;
use crate::classes::e_message_type::EMessageType;

pub struct ConsoleRow{
    message: String,
    message_type: EMessageType,
    message_time: String
}

impl ConsoleRow {
    pub fn get_color(&self) -> impl Into<Color32> {
        match self.message_type {
            EMessageType::Log => {
                egui::Color32::LIGHT_GRAY
            }
            EMessageType::Warning => {
                egui::Color32::YELLOW
            }
            EMessageType::Error => {
                egui::Color32::RED
            }
            EMessageType::Success => {
                egui::Color32::GREEN
            }
        }
    }
}

impl ConsoleRow {
    pub fn new(message: String, message_type: EMessageType) -> Self {

        Self{
            message,
            message_type,
            message_time: Local::now().format("%H:%M:%S").to_string()
        }
    }

    pub fn get_message(&self) -> String {
        format!("[{}]: {}", self.message_time, self.message)
    }
}