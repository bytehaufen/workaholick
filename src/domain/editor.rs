use super::NewEntry;
use anyhow::Result;

pub trait Editor {
    fn edit(&mut self, entry: &mut NewEntry) -> Result<()>;
}
