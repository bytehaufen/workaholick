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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn std_env_var_gets_set_and_unset() {
        let key = "WORKAHOLICK_TEST_STDENV_VAR";

        unsafe { env::remove_var(key) };
        assert_eq!(StdEnv.var(key), None);

        unsafe { env::set_var(key, "value") };
        assert_eq!(StdEnv.var(key).as_deref(), Some("value"));

        unsafe { env::remove_var(key) };
        assert_eq!(StdEnv.var(key), None);
    }

    #[test]
    fn std_fs_write_then_read_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("file.txt");

        StdFs.write_string(&path, "hello").unwrap();
        let read = StdFs.read_string(&path).unwrap();

        assert_eq!(read, "hello");
    }

    #[test]
    fn std_fs_read_missing_file_errors() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("missing.txt");

        assert!(StdFs.read_string(&path).is_err());
    }

    #[test]
    fn temp_file_temp_creates_file_with_suffix() {
        let path = TempFileTemp.temp_file("md").unwrap();
        assert!(path.exists());
        assert!(path.to_string_lossy().ends_with(".md"));

        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn std_process_run_success_is_ok() {
        StdProcess.run("true", &[]).unwrap();
    }

    #[test]
    fn std_process_run_failure_is_err() {
        let err = StdProcess.run("false", &[]).unwrap_err();
        assert!(err.to_string().contains("editor exited with"));
    }

    #[test]
    fn std_process_run_missing_program_is_err() {
        assert!(
            StdProcess
                .run("definitely-not-a-real-program", &[])
                .is_err()
        );
    }
}
