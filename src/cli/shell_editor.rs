use crate::editor;
use crate::editor::command::parse_editor_command;
use crate::editor::system::{
    Editor, Env, Fs, Process, StdEnv, StdFs, StdProcess, Temp, TempFileTemp,
};

use anyhow::{Context, Result};
use std::fs;

pub struct ShellEditor<E, F, T, P> {
    env: E,
    fs: F,
    temp: T,
    proc: P,
}

impl<E, F, T, P> ShellEditor<E, F, T, P> {
    pub fn new(env: E, fs: F, temp: T, proc: P) -> Self {
        Self {
            env,
            fs,
            temp,
            proc,
        }
    }
}

impl ShellEditor<StdEnv, StdFs, TempFileTemp, StdProcess> {
    pub fn real() -> Self {
        Self::new(StdEnv, StdFs, TempFileTemp, StdProcess)
    }
}

impl<E: Env, F: Fs, T: Temp, P: Process> Editor for ShellEditor<E, F, T, P> {
    fn edit(&self, initial: &str) -> Result<String> {
        let path = self.temp.temp_file("toml").context("creating temp file")?;

        self.fs
            .write_string(&path, initial)
            .context("writing temp file")?;

        let cmd = editor::command::choose_editor(self.env.var("VISUAL"), self.env.var("EDITOR"));
        let mut cmd = parse_editor_command(&cmd)?;

        // Final arg is file path
        cmd.args.push(path.to_string_lossy().into());

        self.proc
            .run(&cmd.program, &cmd.args)
            .context("running editor")?;

        let edited = self.fs.read_string(&path).context("reading edited file")?;

        let _ = fs::remove_file(&path);

        Ok(edited)
    }
}
