#[derive(Default)]
pub struct SelectProcessWindow {
    pub show_process_selector: bool,
    pub pending_process_selection: Option<u32>,
    pub show_reset_warning: bool,
    pub requested_process_selection: Option<u32>,
    pub close_selector_on_apply: bool,
}

impl SelectProcessWindow {
    pub fn open(&mut self, current_selection: Option<u32>) {
        self.pending_process_selection = current_selection;
        self.show_process_selector = true;
    }

    pub fn close(&mut self) {
        self.show_process_selector = false;
    }

    pub fn request_change(&mut self, target_selection: Option<u32>, close_selector_on_apply: bool) {
        self.requested_process_selection = target_selection;
        self.close_selector_on_apply = close_selector_on_apply;
        self.show_reset_warning = true;
    }

    pub fn clear_request(&mut self) {
        self.show_reset_warning = false;
        self.requested_process_selection = None;
        self.close_selector_on_apply = false;
    }
}
