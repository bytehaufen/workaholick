use crate::domain::ActivityKind;

#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub enum DayState {
    #[default]
    Idle,
    Active(ActivityKind),
    Breaking(ActivityKind),
}
