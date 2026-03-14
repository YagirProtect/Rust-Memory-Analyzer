#[derive(Debug, Clone)]
pub struct MemoryRegion {
    pub base_address: usize,
    pub allocation_base: usize,
    pub region_size: usize,
    pub state: u32,
    pub protect: u32,
    pub region_type: u32,
}