#[derive(Clone)]
pub enum ProcEvent {
    Start { pid: u32, name: String },
    Exit(u32),
}
