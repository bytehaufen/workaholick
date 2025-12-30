use super::NewEntry;
use anyhow::Result;

pub trait Editor {
    fn edit(&self, entry: &mut NewEntry) -> Result<()>;
}
