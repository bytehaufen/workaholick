use crate::domain::WorkContext;

#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub enum DayState {
    #[default]
    Idle,
    Active(WorkContext),
    Breaking(WorkContext),
}
