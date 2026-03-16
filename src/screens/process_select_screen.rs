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
use crate::classes::c_scan_result_row::ResultRow;
use crate::classes::e_message_type::EMessageType;
use crate::classes::e_value_type::EValueType;
use egui_extras::{Column, TableBuilder};

impl App {
    pub fn draw(&mut self, ctx: &Context, ui: &mut Ui) {
        if self.processes.selected_process.is_none() {
            self.draw_empty_process_state(ui);
        } else {
            self.draw_selected_process_details(ctx, ui);
        }

        self.draw_process_selector_window(ctx);
        self.draw_reset_warning_window(ctx);
    }

    fn apply_process_selection(&mut self, selection: Option<u32>) {
        if self.processes.selected_process != selection {
            self.opened_process = None;
        }
        self.processes.selected_process = selection;
    }

    fn request_process_selection(
        &mut self,
        selection: Option<u32>,
        close_selector_on_apply: bool,
    ) -> bool {
        if self.processes.selected_process == selection {
            if close_selector_on_apply {
                self.select_process_window.close();
            }
            return true;
        }

        if self.opened_process.is_some() {
            self.select_process_window
                .request_change(selection, close_selector_on_apply);
            return false;
        }

        self.apply_process_selection(selection);
        if close_selector_on_apply {
            self.select_process_window.close();
        }
        true
    }

    fn draw_empty_process_state(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            let top_space = (ui.available_height() * 0.35).max(40.0);
            ui.add_space(top_space);
            ui.heading("No process selected");
            ui.add_space(8.0);

            if ui.button("Select a process").clicked() {
                self.select_process_window
                    .open(self.processes.selected_process);
            }
        });
    }

    fn draw_process_selector_window(&mut self, ctx: &Context) {
        if !self.select_process_window.show_process_selector {
            return;
        }

        let viewport = ctx.content_rect();
        let window_height = (viewport.height() * 0.8).max(320.0);
        let window_width = (viewport.width() * 0.55).clamp(420.0, 920.0);

        let mut is_open = self.select_process_window.show_process_selector;
        let mut close_requested = false;
        let mut pending_selection = self.select_process_window.pending_process_selection;

        egui::Window::new("Select Process")
            .open(&mut is_open)
            .collapsible(false)
            .resizable(true)
            .default_size([window_width, window_height])
            .max_height(window_height)
            .show(ctx, |ui| {
                self.draw_process_selector_content(ui, &mut pending_selection);

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        close_requested = true;
                    }

                    let can_select = pending_selection.is_some();
                    if ui
                        .add_enabled(can_select, egui::Button::new("Select"))
                        .clicked()
                    {
                        if self.request_process_selection(pending_selection, true) {
                            close_requested = true;
                        }
                    }
                });
            });

        self.select_process_window.pending_process_selection = pending_selection;

        if close_requested || !is_open {
            self.select_process_window.close();
        } else {
            self.select_process_window.show_process_selector = true;
        }
    }

    fn draw_reset_warning_window(&mut self, ctx: &Context) {
        if !self.select_process_window.show_reset_warning {
            return;
        }

        let mut confirm = false;
        let mut cancel = false;

        egui::Modal::new(egui::Id::new("reset_warning_modal")).show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.heading("Warning");
                ui.label("Opened process will be closed and scan state will be reset.");
                ui.label("Continue?");
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                    if ui.button("Continue").clicked() {
                        confirm = true;
                    }
                });
            });
        });

        if confirm {
            let selection = self.select_process_window.requested_process_selection;
            let close_selector_on_apply = self.select_process_window.close_selector_on_apply;

            self.apply_process_selection(selection);

            if close_selector_on_apply {
                self.select_process_window.close();
            }

            self.select_process_window.clear_request();
        } else if cancel {
            self.select_process_window.clear_request();
        }
    }

    fn draw_process_selector_content(
        &mut self,
        ui: &mut Ui,
        pending_selection: &mut Option<u32>,
    ) {
        ui.horizontal(|ui| {
            ui.label("🔍");

            ui.add(
                egui::TextEdit::singleline(&mut self.processes.search).hint_text("name / pid / exe"),
            );

            if ui.button("❌").clicked() {
                self.processes.search = String::new();
            }
        });

        ui.separator();

        egui::ScrollArea::vertical()
            .max_width(f32::INFINITY)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

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

                    let mut header =
                        egui::CollapsingHeader::new(format!("{} ({})", name, list.len()))
                            .id_salt(format!("group_{name}"));

                    if should_open_for_pending {
                        header = header.open(Some(true));
                    } else if !query.is_empty() {
                        header = header.default_open(true);
                    }

                    header.show(ui, |ui| {
                        for p in list {
                            let selected = *pending_selection == Some(p.pid);

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
                                *pending_selection = Some(p.pid);
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

                if did_scroll {
                    self.processes.pending_scroll_to_pid = None;
                }
            });
    }
    fn draw_selected_process_details(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Process Details");
            if ui.button("Select a process").clicked() {
                self.select_process_window
                    .open(self.processes.selected_process);
            }
        });
        ui.separator();

        let Some(selected_pid) = self.processes.selected_process else {
            ui.label("No process selected");
            return;
        };

        let (process_pid, name, parent_pid, exe_path, cwd) = {
            let Some(process) = self.system.process(Pid::from_u32(selected_pid)) else {
                ui.colored_label(Color32::YELLOW, "Selected process no longer exists");
                return;
            };

            (
                process.pid().as_u32(),
                process.name().to_string_lossy().to_string(),
                process.parent().map(|p| p.as_u32()),
                process.exe().map(|p| p.display().to_string()).unwrap_or_default(),
                process.cwd().map(|p| p.display().to_string()).unwrap_or_default(),
            )
        };

        let has_parent = self.processes.is_process_has_parent(self.processes.selected_process);

        ui.horizontal(|ui| {
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
                            let _ = self.request_process_selection(Some(ppid), false);
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

            let terminate_button = ui.button(RichText::new("Terminate").color(Color32::RED));

            if terminate_button.clicked() {
                let _ = utils::terminate_process_by_pid(selected_pid);
            }

            let open_process_button = ui.button(RichText::new("Open process").color(Color32::RED));

            if open_process_button.clicked() {
                let open = OpenedProcess::new(process_pid);

                match open {
                    Ok(p) => {
                        self.console.add_message(ConsoleRow::new(
                            format!("Process [{}] is opened", process_pid),
                            EMessageType::Success,
                        ));
                        self.opened_process = Some(p);
                    }
                    Err(e) => {
                        self.console
                            .add_message(ConsoleRow::new(e.to_string(), EMessageType::Error))
                    }
                }
            }
        });

        self.draw_registers_table(ctx, ui);
        self.draw_console_log(ctx, ui);
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

    fn draw_registers_table(&mut self, _ctx: &Context, ui: &mut Ui) {
        if let Some(process) = &mut self.opened_process {
            ui.separator();
            ui.heading("Scanner");
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Value:");

                ui.add(
                    egui::TextEdit::singleline(&mut process.scan.input_value)
                        .hint_text("Enter value")
                        .desired_width(140.0),
                );
                
                

                egui::ComboBox::from_label("Type")
                    .selected_text(format!("{:?}", process.scan.selected_value_type))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut process.scan.selected_value_type, EValueType::I32, "I32");
                        ui.selectable_value(&mut process.scan.selected_value_type, EValueType::I64, "I64");
                        ui.selectable_value(&mut process.scan.selected_value_type, EValueType::F32, "F32");
                        ui.selectable_value(&mut process.scan.selected_value_type, EValueType::F64, "F64");
                    });

                if ui.button("Full Scan").clicked() {
                    process.full_scan(&mut self.console, _ctx);
                }

                let next_enabled = process.scan.has_scan_session;
                if ui
                    .add_enabled(next_enabled, egui::Button::new("Next Scan"))
                    .clicked()
                {
                    process.next_scan();
                }

                if ui.button("Reset").clicked() {
                    process.reset_scan();
                }
                

                ui.label("Displayed rows:");
                let max_rows_response = ui.add(
                    egui::TextEdit::singleline(&mut process.scan.scan_results_count_input)
                        .desired_width(90.0)
                        .hint_text("10000"),
                );

                if max_rows_response.changed() {
                    if let Ok(parsed) = process.scan.scan_results_count_input.trim().parse::<usize>() {
                        process.scan.scan_results_count = parsed.max(1);
                    }
                }
            });

            ui.label(format!("Scan results: {}", process.scan.results.len()));

            ui.separator();
            ui.heading("Scan Results");
            ui.separator();

            process.pump_scan_messages(&mut self.console);

            if process.scan.results.is_empty() {
                ui.label("No scan results");
            } else {
                Self::draw_table(ui, &process.scan.results, ("scan_results_table", process.pid), process.scan.scan_results_count, false);
            }
        }
    }

    fn draw_table(ui: &mut Ui, results: &[ResultRow], table_id: (&str, u32), display_count: usize, has_description: bool) {
        let row_height = 24.0;
        let header_height = 24.0;
        let available_width = ui.available_width().max(320.0);
        let max_scroll_height = 260.0;//(ui.available_height() * 0.85).max(260.0);

        let address_width = (available_width * 0.25).clamp(110.0, 260.0);
        let type_width = (available_width * 0.14).clamp(76.0, 120.0);
        let value_width = (available_width * 0.24).clamp(96.0, 220.0);
        let action_width = 72.0;

        let base_table = TableBuilder::new(ui)
            .id_salt(table_id)
            .striped(true)
            .resizable(false)
            .cell_layout(egui::Layout::left_to_right(Align::Center))
            .min_scrolled_height(140.0)
            .max_scroll_height(max_scroll_height);

        let table = if has_description {
            base_table
                .column(Column::remainder().at_least(120.0).clip(true).resizable(false))
                .column(Column::initial(address_width).at_least(110.0).at_most(available_width * 0.4).clip(true).resizable(false))
                .column(Column::initial(type_width).at_least(76.0).at_most(120.0).clip(true).resizable(false))
                .column(Column::initial(value_width).at_least(96.0).at_most(220.0).clip(true).resizable(false))
                .column(Column::initial(action_width).at_least(64.0).at_most(80.0).clip(true).resizable(false))
        } else {
            base_table
                .column(Column::remainder().at_least(120.0).clip(true).resizable(false))
                .column(Column::initial(type_width).at_least(76.0).at_most(120.0).clip(true).resizable(false))
                .column(Column::initial(value_width).at_least(96.0).at_most(220.0).clip(true).resizable(false))
                .column(Column::initial(action_width).at_least(64.0).at_most(80.0).clip(true).resizable(false))
        };

        table
            .header(header_height, |mut header| {
                if has_description {
                    header.col(|ui| {
                        ui.strong("Description");
                    });
                }
                header.col(|ui| {
                    ui.strong("Address");
                });
                header.col(|ui| {
                    ui.strong("Type");
                });
                header.col(|ui| {
                    ui.strong("Value");
                });
                header.col(|ui| {
                    ui.strong("Action");
                });
            })
            .body(|body| {
                body.rows(row_height, results.len().min(display_count), |mut row| {
                    let item = &results[row.index()];

                    if has_description {
                        row.col(|ui| {
                            ui.label(item.description.as_deref().unwrap_or("-"));
                        });
                    }
                    row.col(|ui| {
                        ui.monospace(format!("0x{:X}", item.address));
                    });
                    row.col(|ui| {
                        ui.label(format!("{:?}", item.value_type));
                    });
                    row.col(|ui| {
                        ui.label(item.cached_value.as_str());
                    });
                    row.col(|ui| {
                        if ui.button("Add").clicked() {
                            // add
                        }
                    });
                });
            });
    }
}



