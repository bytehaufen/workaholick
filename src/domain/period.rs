#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Period {
    Today,
    Prev,
    Week,
    Month,
}
