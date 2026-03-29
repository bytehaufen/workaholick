#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Period {
    Today,
    LastDay,
    Week,
    Month,
    Year,
}
