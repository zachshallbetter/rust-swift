#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MachineStatus {
    Starting,
    Running,
    Halted,
}

pub fn advance(status: MachineStatus) -> MachineStatus {
    match status {
        MachineStatus::Starting => MachineStatus::Running,
        MachineStatus::Running => MachineStatus::Halted,
        MachineStatus::Halted => MachineStatus::Starting,
    }
}
