#[derive(Debug)]
pub enum Command {
    In {
        description: String,
        // maybe later: place: Place, date: NaiveDate, ...
    },
    Out,
    Status,
    // whatever else your domain needs
}
