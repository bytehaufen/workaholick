#[derive(Debug)]
pub enum Command {
    In { description: String },
    Out,
    Status,
}
