use anyhow::{Result, bail};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::Builder;

pub trait Editor {
    fn edit(&self, initial: &str) -> Result<String>;
}

pub trait Env {
    fn var(&self, key: &str) -> Option<String>;
}
pub struct StdEnv;
impl Env for StdEnv {
    fn var(&self, key: &str) -> Option<String> {
        env::var(key).ok()
    }
}

pub trait Fs {
    fn write_string(&self, path: &Path, content: &str) -> Result<()>;
    fn read_string(&self, path: &Path) -> Result<String>;
}
pub struct StdFs;
impl Fs for StdFs {
    fn write_string(&self, path: &Path, content: &str) -> Result<()> {
        fs::write(path, content)?;
        Ok(())
    }

    fn read_string(&self, path: &Path) -> Result<String> {
        Ok(fs::read_to_string(path)?)
    }
}

pub trait Temp {
    fn temp_file(&self, extension: &str) -> Result<PathBuf>;
}
pub struct TempFileTemp;
impl Temp for TempFileTemp {
    fn temp_file(&self, extension: &str) -> Result<PathBuf> {
        let file = Builder::new().suffix(&format!(".{extension}")).tempfile()?;

        let (_file, path) = file.keep()?;
        Ok(path)
    }
}

pub trait Process {
    fn run(&self, program: &str, args: &[String]) -> Result<()>;
}
pub struct StdProcess;
impl Process for StdProcess {
    fn run(&self, program: &str, args: &[String]) -> Result<()> {
        let status = Command::new(program).args(args).status()?;
        if !status.success() {
            bail!("editor exited with {status}");
        }

        Ok(())
    }
}
