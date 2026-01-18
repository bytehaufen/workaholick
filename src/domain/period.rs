#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Period {
    Today,
    PrevDay,
    Week,
    Month,
}
