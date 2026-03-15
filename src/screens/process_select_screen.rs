use crate::app::App;
use crate::classes::c_process_row::ProcessRow;
use crate::utils;
use eframe::egui;
use eframe::egui::{Align, Color32, Context, RichText, Ui};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use sysinfo::Pid;
use crate::classes::c_console_row::ConsoleRow;
use crate::classes::c_opened_process::OpenedProcess;
use crate::classes::e_message_type::EMessageType;

impl App {
    pub fn draw(&mut self, ctx: &Context, ui: &mut Ui) {
        self.draw_process_list(ctx);
        self.draw_selected_process_details(ctx, ui);
    }

    fn draw_process_list(&mut self, ctx: &Context) {
        egui::SidePanel::left("processes_list").show(ctx, |ui| {
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("🔍");

                ui.add(
                    egui::TextEdit::singleline(&mut self.processes.search)
                        .hint_text("name / pid / exe")
                );

                if (ui.button("❌").clicked()) {
                    self.processes.search = String::new();
                }
            });

            ui.separator();

            egui::ScrollArea::vertical()
                .max_width(f32::INFINITY)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    let mut target_id = self.processes.selected_process.unwrap_or(0);
                    let pending_scroll_to_pid = self.processes.pending_scroll_to_pid;

                    let query = self.processes.search.trim().to_lowercase();
                    let mut groups: BTreeMap<String, Vec<&ProcessRow>> = BTreeMap::new();

                    for p in self.processes.get_processes(&mut self.system) {
                        let matches = if query.is_empty() {
                            true
                        } else {
                            p.name.to_lowercase().contains(&query)
                                || p.exe.to_lowercase().contains(&query)
                                || p.pid.to_string().contains(&query)
                        };

                        if matches {
                            groups.entry(p.name.clone()).or_default().push(p);
                        }
                    }

                    let mut did_scroll = false;

                    let should_open_for_pending = pending_scroll_to_pid.is_some();

                    for (name, list) in groups {
                        let pid_set: HashSet<u32> = list.iter().map(|p| p.pid).collect();

                        let mut header = egui::CollapsingHeader::new(format!("{} ({})", name, list.len()))
                            .id_salt(format!("group_{name}"));


                        if should_open_for_pending {
                            header = header.open(Some(true));
                        } else if !query.is_empty() {
                            header = header.default_open(true);
                        }


                        header.show(ui, |ui| {
                            for p in list {
                                let selected = target_id == p.pid;

                                let exe_name = Path::new(&p.exe)
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .filter(|s| !s.is_empty())
                                    .unwrap_or(&p.name);

                                let is_root = match p.parent_pid {
                                    Some(parent_pid) => !pid_set.contains(&parent_pid),
                                    None => true,
                                };

                                let label_text = if is_root {
                                    format!("[{}] {}  🔑", p.pid, exe_name)
                                } else {
                                    format!("[{}] {}", p.pid, exe_name)
                                };

                                let text = if is_root {
                                    egui::RichText::new(label_text)
                                        .color(egui::Color32::LIGHT_GREEN)
                                        .strong()
                                } else {
                                    egui::RichText::new(label_text)
                                };

                                let response = ui.selectable_label(selected, text);

                                if response.clicked() {
                                    target_id = p.pid;
                                }
                                if pending_scroll_to_pid == Some(p.pid) {
                                    response.scroll_to_me(Some(Align::Center));
                                    did_scroll = true;
                                }

                                if !p.exe.is_empty() {
                                    response.on_hover_text(&p.exe);
                                }
                            }
                        });
                    }

                    self.processes.selected_process = Some(target_id);

                    if did_scroll {
                        self.processes.pending_scroll_to_pid = None;
                    }
                });
        });
    }
    fn draw_selected_process_details(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Process Details");
            ui.separator();


            let Some(selected_pid) = self.processes.selected_process else {
                ui.label("No process selected");
                return;
            };

            let Some(process) = self.system.process(Pid::from_u32(selected_pid)) else {
                ui.colored_label(Color32::YELLOW, "Selected process no longer exists");
                return;
            };


            let has_parent = self.processes.is_process_has_parent(self.processes.selected_process);
            let name = process.name().to_string_lossy().to_string();
            let parent_pid = process.parent().map(|p| p.as_u32());
            let exe_path = process.exe().map(|p| p.display().to_string()).unwrap_or_default();
            let cwd = process.cwd().map(|p| p.display().to_string()).unwrap_or_default();


            ui.horizontal(|ui| {
                // Сюда потом подставишь иконку
                ui.vertical(|ui| {
                    ui.label(RichText::new(&name).strong().size(22.0));
                    ui.label(format!("PID: {}", selected_pid));

                    if let Some(ppid) = parent_pid {
                        ui.horizontal(|ui| {
                            ui.label(format!("Parent PID: {}", ppid));

                            let to_parent = ui
                                .add_enabled(has_parent, egui::Button::new("Go to parent"))
                                .clicked();

                            if to_parent {
                                self.processes.selected_process = Some(ppid);
                                self.processes.pending_scroll_to_pid = Some(ppid);
                            }
                        });
                    } else {
                        ui.label("Parent PID: None");
                    }
                });
            });

            ui.separator();

            ui.label(RichText::new("Executable").strong());
            if exe_path.is_empty() {
                ui.colored_label(Color32::GRAY, "Unavailable");
            } else {
                ui.monospace(&exe_path);
            }

            ui.add_space(8.0);

            ui.label(RichText::new("Working directory").strong());
            if cwd.is_empty() {
                ui.colored_label(Color32::GRAY, "Unavailable");
            } else {
                ui.monospace(&cwd);
            }

            ui.add_space(12.0);

            ui.horizontal_wrapped(|ui| {
                let open_clicked = ui
                    .add_enabled(!exe_path.is_empty(), egui::Button::new("Open in Explorer"))
                    .clicked();

                if open_clicked {
                    let _ = utils::open_in_explorer(&exe_path);
                }


                let terminate_button = ui.button(
                    RichText::new("Terminate").color(Color32::RED)
                );

                if terminate_button.clicked() {
                    let _ = utils::terminate_process_by_pid(selected_pid);
                }


                let open_process_button = ui.button(
                    RichText::new("Open process").color(Color32::RED)
                );
                
                if open_process_button.clicked() {
                    let open = OpenedProcess::new(process.pid().as_u32());
                    
                    match open {
                        Ok(p) => {
                            self.console.add_message(ConsoleRow::new(format!("Process [{}] is opened", process.pid().as_u32()), EMessageType::Success));

                            match p.enumerate_regions(){
                                Ok(regions) => {
                                    for region in regions.iter().take(10) {
                                        let v = format!(
                                            "base=0x{:X}, alloc_base=0x{:X}, size={}, state=0x{:X}, protect=0x{:X}, type=0x{:X}",
                                            region.base_address,
                                            region.allocation_base,
                                            region.region_size,
                                            region.state,
                                            region.protect,
                                            region.region_type,
                                        );

                                        self.console.add_message(ConsoleRow::new(v, EMessageType::Success))

                                    }

                                    
                                }
                                Err(e) => {
                                    self.console.add_message(ConsoleRow::new(e.to_string(), EMessageType::Error))
                                }
                            }


                            self.opened_process = Some(p);
                        }
                        Err(e) => {
                            self.console.add_message(ConsoleRow::new(e.to_string(), EMessageType::Error))
                        }
                    }
                }
            });



            self.draw_console_log(ctx, ui);
        });
    }

    fn draw_console_log(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {

        let collapsed_h = 28.0;
        let expanded_h = 180.0;

        let height = if self.console.is_active() {
            expanded_h
        } else {
            collapsed_h
        };

        egui::TopBottomPanel::bottom("console_panel")
            .exact_height(height)
            .resizable(self.console.is_active())
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let arrow = if self.console.is_active() { "🔽" } else { "▶" };

                    if ui.button(arrow).clicked() {
                        self.console.set_active(!self.console.is_active());
                    }

                    ui.label("Console");


                    let pin_label = if self.console.is_pinned_to_bottom() {
                        "Unpin"
                    } else {
                        "Pin to bottom"
                    };

                    if ui.button(pin_label).clicked() {
                        self.console.set_pinned_to_bottom(!self.console.is_pinned_to_bottom());
                    }

                    if ui.button("Clear").clicked() {
                        self.console.clear();
                    }
                });

                if self.console.is_active() {
                    ui.separator();

                    let frame = egui::Frame::NONE.show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("console_scroll")
                            .max_width(f32::INFINITY)
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());

                                let mut last_response = None;

                                for line in self.console.get_messages() {
                                    let response = ui.label(
                                        egui::RichText::new(line.get_message())
                                            .monospace()
                                            .color(line.get_color())
                                    );
                                    last_response = Some(response);
                                }

                                if self.console.is_pinned_to_bottom() || self.console.jump_to_bottom_once {
                                    if let Some(response) = last_response {
                                        response.scroll_to_me(Some(egui::Align::BOTTOM));
                                    }
                                    self.console.clear_jump_request();
                                }
                            });
                    });

                    let scrolled_up = ctx.input(|i| i.raw_scroll_delta.y > 0.0);

                    if self.console.is_pinned_to_bottom()
                        && frame.response.contains_pointer()
                        && scrolled_up
                    {
                        self.console.set_pinned_to_bottom(false);
                    }
                }
            });
    }
}



