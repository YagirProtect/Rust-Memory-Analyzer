use crate::classes::e_value_type::EValueType;
use std::sync::mpsc::Receiver;

#[derive(Debug, Clone)]
pub struct ResultRow {
    pub description: Option<String>,
    pub address: usize,
    pub value_type: EValueType,
    pub cached_value: String,
}

pub struct ScanState {
    pub input_value: String,
    pub results: Vec<ResultRow>,
    pub has_scan_session: bool,
    pub selected_value_type: EValueType,
    pub scan_results_count: usize,
    pub scan_results_count_input: String,
}

impl Default for ScanState {
    fn default() -> Self {
        Self{
            input_value: "".to_string(),
            results: vec![],
            has_scan_session: false,
            selected_value_type: Default::default(),
            scan_results_count: 10_000,
            scan_results_count_input: "10000".to_string(),
        }
    }
}

pub enum ScanMessage {
    Started { total_regions: usize },
    Progress { scanned_regions: usize, found: usize },
    Found(ResultRow),
    Done { found: usize },
    Error(String),
}

#[derive(Default)]
pub struct AsyncProcessScan {
    pub is_running: bool,
    pub total_regions: usize,
    pub scanned_regions: usize,
    pub total_found: usize,
    pub receiver: Option<Receiver<ScanMessage>>,
}
