use sysinfo::{ProcessesToUpdate, System};
use crate::classes::c_process_row::ProcessRow;

#[derive(Default)]
pub struct Processes {
    processes: Vec<ProcessRow>,
    pub selected_process: Option<u32>,
    pub search: String,

    pub last_update_time: u32,
    pub pending_scroll_to_pid: Option<u32>,
}

impl Processes {
    pub fn refresh_processes(&mut self, system: &mut System) {
        system.refresh_processes(ProcessesToUpdate::All, true);
        self.processes.clear();
        for (pid, process) in system.processes() {
            let process = ProcessRow {
                pid: pid.as_u32(),
                name: process.name().to_string_lossy().to_string(),
                exe: process
                    .exe()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
                parent_pid: process.parent().map(|p| p.as_u32()),
            };
            self.processes.push(process);
        }

        self.processes.sort_by(|a, b| a.name.cmp(&b.name));
    }

    pub fn get_processes(&mut self, system: &mut System) -> &Vec<ProcessRow> {

        let current = std::time::UNIX_EPOCH.elapsed().unwrap().as_millis() as u32;

        if (self.processes.len() == 0 || current - self.last_update_time > 2){
            self.refresh_processes(system);
        }

        self.last_update_time = current;

        &self.processes
    }

    pub fn is_process_has_parent(&self, pid: Option<u32>) -> bool {
        if (pid.is_none())
        {
            return false;
        }

        let val = self.processes.iter().find(|p| p.pid == pid.unwrap());

        if (val.is_some()){
            let pid = val.unwrap().parent_pid;
            if (pid.is_some()){
                let parent_pid = pid.unwrap();
                
                return self.processes.iter().any(|p| p.pid == parent_pid);
            }
        }


        return false;
    }
}