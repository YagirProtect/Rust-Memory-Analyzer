#[derive(Default, Clone)]
pub struct ProcessRow {
    pub pid: u32,
    pub name: String,
    pub exe: String,
    pub parent_pid: Option<u32>,
}