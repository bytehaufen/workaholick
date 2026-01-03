use crate::editor;
use crate::editor::command::parse_editor_command;
use crate::editor::system::{
    Editor, Env, FileType, Fs, Process, StdEnv, StdFs, StdProcess, Temp, TempFileTemp,
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
    fn edit(&self, initial: &str, file_type: FileType) -> Result<String> {
        let path = self
            .temp
            .temp_file(&file_type.to_string())
            .context("creating temp file")?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Result, anyhow};
    use std::{
        cell::RefCell,
        collections::HashMap,
        path::{Path, PathBuf},
        rc::Rc,
    };

    #[derive(Default)]
    struct FakeEnv {
        vars: HashMap<String, Option<String>>,
    }
    impl FakeEnv {
        fn with(mut self, key: &str, value: Option<&str>) -> Self {
            self.vars
                .insert(key.to_string(), value.map(|string| string.to_string()));
            self
        }
    }
    impl Env for FakeEnv {
        fn var(&self, key: &str) -> Option<String> {
            self.vars.get(key).cloned().unwrap_or(None)
        }
    }

    #[derive(Clone, Default)]
    struct FakeFs {
        files: Rc<RefCell<HashMap<PathBuf, String>>>,
    }
    impl Fs for FakeFs {
        fn write_string(&self, path: &Path, content: &str) -> Result<()> {
            self.files
                .borrow_mut()
                .insert(path.to_path_buf(), content.to_string());
            Ok(())
        }

        fn read_string(&self, path: &Path) -> Result<String> {
            self.files
                .borrow()
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow!("missing file: {}", path.display()))
        }
    }

    struct FakeTemp {
        path: PathBuf,
    }
    impl Temp for FakeTemp {
        fn temp_file(&self, _extension: &str) -> Result<PathBuf> {
            Ok(self.path.clone())
        }
    }

    #[derive(Default)]
    struct FakeProc {
        calls: RefCell<Vec<(String, Vec<String>)>>,
        fail: bool,
        fs: FakeFs,
        edited: String,
    }
    impl Process for FakeProc {
        fn run(&self, program: &str, args: &[String]) -> Result<()> {
            self.calls
                .borrow_mut()
                .push((program.to_string(), args.to_vec()));
            if self.fail {
                return Err(anyhow!("process failed"));
            }

            // Simulate the editor modifying the file
            if let Some(last) = args.last() {
                let path = PathBuf::from(last);
                self.fs.write_string(&path, &self.edited)?;
            }

            Ok(())
        }
    }

    #[test]
    fn edit_writes_initial_opens_editor_reads_back() {
        let env = FakeEnv::default()
            .with("VISUAL", Some("nvim"))
            .with("EDITOR", Some("vi"));
        let fs = FakeFs::default();
        let temp_path = PathBuf::from("/tmp/workaholick-test.ron");
        let temp = FakeTemp {
            path: temp_path.clone(),
        };
        let proc = FakeProc {
            fs: fs.clone(),
            edited: "edited".to_string(),
            ..Default::default()
        };

        let editor = ShellEditor::new(env, fs, temp, proc);

        let out = editor.edit("initial", FileType::Ron).unwrap();
        assert_eq!(out, "edited");

        let calls = editor.proc.calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "nvim");
        assert_eq!(
            calls[0].1.last().unwrap(),
            &temp_path.to_string_lossy().to_string()
        );
    }

    #[test]
    fn edit_uses_editor_when_visual_missing() {
        let env = FakeEnv::default()
            .with("VISUAL", None)
            .with("EDITOR", Some("vim"));
        let fs = FakeFs::default();
        let temp_path = PathBuf::from("/tmp/workaholick-test2.ron");
        let temp = FakeTemp {
            path: temp_path.clone(),
        };
        let proc = FakeProc::default();

        let editor = ShellEditor::new(env, fs, temp, proc);

        editor
            .fs
            .write_string(&temp_path, "edited")
            .expect("seed edited content");

        let _ = editor.edit("initial", FileType::Toml).unwrap();

        let calls = editor.proc.calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "vim");
    }

    #[test]
    fn edit_returns_error_when_process_fails() {
        let env = FakeEnv::default()
            .with("VISUAL", Some("nvim"))
            .with("EDITOR", Some("vi"));
        let fs = FakeFs::default();
        let temp_path = PathBuf::from("/tmp/workaholick-test3.ron");
        let temp = FakeTemp {
            path: temp_path.clone(),
        };
        let proc = FakeProc {
            fail: true,
            ..Default::default()
        };
        let editor = ShellEditor::new(env, fs, temp, proc);

        let err = editor.edit("initial", FileType::Ron).unwrap_err();
        assert!(err.to_string().contains("running editor"));
    }
}
