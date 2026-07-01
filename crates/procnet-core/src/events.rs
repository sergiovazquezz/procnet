#[derive(Clone)]
pub struct ProcStartEvent {
    pub pid: u32,
    pub name: String,
}
