use crate::classes::c_console_row::ConsoleRow;


const MESSAGE_COUNT_LIMIT: usize = 2048;

pub struct Console {
    is_active: bool,
    list: Vec<ConsoleRow>,

    pub auto_stick_to_bottom: bool,
    pub jump_to_bottom_once: bool,
}

impl Default for Console {
    fn default() -> Self {
        Self{
            is_active: false,
            list: vec![],
            auto_stick_to_bottom: true,
            jump_to_bottom_once: true,
        }
    }
}

impl Console {
    pub fn add_message(&mut self, msg: ConsoleRow) {
        self.list.push(msg);
        if (self.list.len() >= MESSAGE_COUNT_LIMIT){
            self.list.remove(0);
        }
    }

    pub fn get_messages(&self) -> &Vec<ConsoleRow> {
        &self.list
    }

    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn set_pinned_to_bottom(&mut self, pinned: bool) {
        self.auto_stick_to_bottom = pinned;
    }

    pub fn is_pinned_to_bottom(&self) -> bool {
        self.auto_stick_to_bottom
    }

    pub fn jump_to_bottom(&mut self) {
        self.jump_to_bottom_once = true;
        self.auto_stick_to_bottom = true;
    }

    pub fn clear_jump_request(&mut self) {
        self.jump_to_bottom_once = false;
    }

    pub fn clear(&mut self) {
        self.list.clear();
    }
}