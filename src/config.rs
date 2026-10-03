use std::{
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

use directories::UserDirs;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    domain::WeeklySchedule,
    editor::{Editor, EditorError, ports::FileType},
};

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const CONFIG_NAME: &str = "config";

#[derive(Debug, thiserror::Error)]
pub(crate) enum ConfigError {
    #[error("no home directory is availableto determine the default store path")]
    MissingHomeDirectory,

    #[error("no config file at {path}")]
    MissingConfigFile { path: PathBuf },

    #[error("could not determine the config file path")]
    ConfigPath {
        #[source]
        source: confy::ConfyError,
    },

    #[error("config file already present at {path}")]
    AlreadyExists { path: PathBuf },

    #[error("could not create directory {path}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not read config file {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not write config file {path}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not parse config file {path}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("could not render config file content for {path}")]
    RenderConfig {
        path: PathBuf,
        #[source]
        source: toml::ser::Error,
    },

    #[error("could not edit config file {path}")]
    Edit {
        path: PathBuf,
        #[source]
        source: EditorError,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(default)]
pub struct Config {
    pub store_path: PathBuf,
    pub schedule: ScheduleConfig,
    pub cli: CliConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ScheduleConfig {
    mon_minutes: i64,
    tue_minutes: i64,
    wed_minutes: i64,
    thu_minutes: i64,
    fri_minutes: i64,
    sat_minutes: i64,
    sun_minutes: i64,
}

impl ScheduleConfig {
    fn as_array(&self) -> [i64; 7] {
        [
            self.mon_minutes,
            self.tue_minutes,
            self.wed_minutes,
            self.thu_minutes,
            self.fri_minutes,
            self.sat_minutes,
            self.sun_minutes,
        ]
    }

    pub(crate) fn new(
        mon_minutes: i64,
        tue_minutes: i64,
        wed_minutes: i64,
        thu_minutes: i64,
        fri_minutes: i64,
        sat_minutes: i64,
        sun_minutes: i64,
    ) -> Self {
        Self {
            mon_minutes,
            tue_minutes,
            wed_minutes,
            thu_minutes,
            fri_minutes,
            sat_minutes,
            sun_minutes,
        }
    }

    pub(crate) fn to_weekly_schedule(&self) -> WeeklySchedule {
        WeeklySchedule::new(self.as_array())
    }
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self::new(480, 480, 480, 480, 480, 0, 0)
    }
}

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct CliConfig {
    #[serde(default)]
    pub formatter: OutputFormatter,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormatter {
    #[default]
    Markdown,
}

impl Config {
    fn for_current_user() -> Result<Self, ConfigError> {
        Ok(Self {
            store_path: default_store_path()?,
            schedule: Default::default(),
            cli: CliConfig::default(),
        })
    }
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawConfig {
            store_path: PathBuf,
            #[serde(default)]
            schedule: ScheduleConfig,
            #[serde(default)]
            cli: CliConfig,
        }

        let raw = RawConfig::deserialize(deserializer)?;
        let schedule_hours = raw.schedule.as_array();

        if schedule_hours.iter().any(|&day_hours| day_hours.is_negative()) {
            return Err(serde::de::Error::custom(
                "schedule day hours must be a non-negative number",
            ));
        }

        Ok(Self { store_path: raw.store_path, schedule: raw.schedule, cli: raw.cli })
    }
}

pub(crate) fn init() -> Result<PathBuf, ConfigError> {
    let path = config_path()?;
    init_at(&path)
}

pub(crate) fn load() -> Result<Config, ConfigError> {
    let path = config_path()?;
    load_from(&path)
}

pub(crate) fn show() -> Result<String, ConfigError> {
    let path = config_path()?;
    show_from(&path)
}

pub(crate) fn edit(editor: &dyn Editor) -> Result<(), ConfigError> {
    let path = config_path()?;
    edit_at(&path, editor)
}

pub(crate) fn init_at(path: impl AsRef<Path>) -> Result<PathBuf, ConfigError> {
    let path = path.as_ref();

    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .map_err(|source| ConfigError::CreateDir { path: parent.to_path_buf(), source })?;
    }

    let cfg = Config::for_current_user()?;
    let temporary = prepare_config_file(path, &cfg)?;

    let _file = temporary.persist_noclobber(path).map_err(|error| match error.error.kind() {
        ErrorKind::AlreadyExists => ConfigError::AlreadyExists { path: path.to_path_buf() },
        _ => ConfigError::Write { path: path.to_path_buf(), source: error.error },
    })?;

    Ok(path.to_path_buf())
}

pub(crate) fn load_from(path: impl AsRef<Path>) -> Result<Config, ConfigError> {
    let path = path.as_ref();
    ensure_exists(path)?;

    let contents = fs::read_to_string(path)
        .map_err(|source| ConfigError::Read { path: path.to_path_buf(), source })?;
    toml::from_str(&contents)
        .map_err(|source| ConfigError::Parse { path: path.to_path_buf(), source })
}

pub(crate) fn show_from(path: impl AsRef<Path>) -> Result<String, ConfigError> {
    let path = path.as_ref();
    ensure_exists(path)?;
    fs::read_to_string(path)
        .map_err(|source| ConfigError::Read { path: path.to_path_buf(), source })
}

pub(crate) fn edit_at(path: impl AsRef<Path>, editor: &dyn Editor) -> Result<(), ConfigError> {
    let path = path.as_ref();
    ensure_exists(path)?;
    let materialized = render_config_for_editing(path)?;

    let header = format!(
        "# {APP_NAME} config\n\
         # Save and close the editor to apply changes.\n\
         # Keep the format valid.\n\n"
    );

    let editored = editor
        .edit(&(header + &materialized), FileType::Toml)
        .map_err(|source| ConfigError::Edit { path: path.to_path_buf(), source })?;
    let cfg: Config = toml::from_str(&editored)
        .map_err(|source| ConfigError::Parse { path: path.to_path_buf(), source })?;
    save(path, &cfg)?;

    Ok(())
}

fn save(path: &Path, cfg: &Config) -> Result<(), ConfigError> {
    let temporary = prepare_config_file(path, cfg)?;

    let _file = temporary
        .persist(path)
        .map_err(|error| ConfigError::Write { path: path.to_path_buf(), source: error.error })?;

    Ok(())
}

fn render_config_for_editing(path: &Path) -> Result<String, ConfigError> {
    let cfg = load_from(path)?;
    toml::to_string_pretty(&cfg)
        .map_err(|source| ConfigError::RenderConfig { path: path.to_path_buf(), source })
}

fn config_path() -> Result<PathBuf, ConfigError> {
    confy::get_configuration_file_path(APP_NAME, CONFIG_NAME)
        .map_err(|source| ConfigError::ConfigPath { source })
}

fn ensure_exists(path: &Path) -> Result<(), ConfigError> {
    if !path.exists() {
        return Err(ConfigError::MissingConfigFile { path: path.to_path_buf() });
    }

    Ok(())
}

fn default_store_path() -> Result<PathBuf, ConfigError> {
    let home = UserDirs::new()
        .map(|user_dirs| user_dirs.home_dir().to_path_buf())
        .ok_or(ConfigError::MissingHomeDirectory)?;

    Ok(home.join(APP_NAME).join("data.jsonl"))
}

fn prepare_config_file(path: &Path, cfg: &Config) -> Result<tempfile::NamedTempFile, ConfigError> {
    let contents = toml::to_string_pretty(cfg)
        .map_err(|source| ConfigError::RenderConfig { path: path.to_path_buf(), source })?;

    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let to_write_error = |source| ConfigError::Write { path: path.to_path_buf(), source };

    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(to_write_error)?;

    temporary.write_all(contents.as_bytes()).map_err(to_write_error)?;
    temporary.flush().map_err(to_write_error)?;
    temporary.as_file().sync_all().map_err(to_write_error)?;

    Ok(temporary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{APP_NAME, OutputFormatter};
    use crate::editor::EditorError;
    use crate::editor::ports::{Editor, FileType};
    use crate::test_support::test_config;
    use std::{cell::RefCell, fs, path::PathBuf};

    struct CapturingEditor {
        seen: RefCell<Option<String>>,
    }

    impl CapturingEditor {
        fn new() -> Self {
            Self { seen: RefCell::new(None) }
        }

        fn seen(&self) -> String {
            self.seen.borrow().clone().expect("editor input should be captured")
        }
    }

    impl Editor for CapturingEditor {
        fn edit(&self, initial: &str, file_type: FileType) -> Result<String, EditorError> {
            assert!(matches!(file_type, FileType::Toml));
            let _ = self.seen.replace(Some(initial.to_string()));
            Ok(initial.to_string())
        }
    }

    #[test]
    fn init_load_show_custom_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("custom-config.toml");

        let expected = Config::for_current_user().unwrap();
        let created_path = init_at(&path).unwrap();
        let loaded = load_from(&path).unwrap();
        let shown = show_from(&path).unwrap();

        assert_eq!(created_path, path);
        assert_eq!(loaded.store_path, expected.store_path);
        assert_eq!(loaded.schedule, expected.schedule);
        assert_eq!(loaded.cli.formatter, OutputFormatter::Markdown);
        assert!(shown.contains("[cli]"));
        assert!(shown.contains("formatter"));
        assert!(shown.contains("store_path"));
        assert!(shown.contains("[schedule"));
        assert!(shown.contains("mon_minutes"));
    }

    #[test]
    fn edit_at_materializes_missing_defaults_in_editor_buffer() {
        let dir = tempfile::tempdir().unwrap();
        let path: PathBuf = dir.path().join("partial-config.toml");
        fs::write(&path, "store_path = \"/tmp/work.jsonl\"\n").unwrap();
        let editor = CapturingEditor::new();

        super::edit_at(&path, &editor).unwrap();

        let seen = editor.seen();
        assert!(seen.contains("[cli]"));
        assert!(seen.contains("formatter = \"markdown\""));
        assert!(seen.contains("store_path = \"/tmp/work.jsonl\""));
        assert!(seen.contains("[schedule"));
        assert!(seen.contains("mon_minutes"));
    }

    #[test]
    fn default_schedule_has_correct_sum() {
        let sum: i64 = test_config().schedule.as_array().iter().sum();
        assert_eq!(sum, 5 * 480);
    }

    #[test]
    fn app_name_is_set() {
        assert_eq!(APP_NAME, "workaholick")
    }

    #[test]
    fn init_at_preserves_existing_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("conifg.toml");

        let original = "store_path = \"/tmp/my-existing-records.jsonl\"\n";
        fs::write(&path, original).unwrap();

        let err = init_at(&path).unwrap_err();

        assert!(
            matches!(err, ConfigError::AlreadyExists { path: existing_path } if existing_path == path)
        );

        assert_eq!(fs::read(&path).unwrap(), original.as_bytes());
    }

    #[test]
    #[cfg(unix)]
    fn save_preserves_existing_config_when_write_fails() {
        use std::{fs::Permissions, os::unix::fs::PermissionsExt};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let original_config = test_config();
        save(&path, &original_config).unwrap();

        let original_bytes = fs::read(&path).unwrap();

        let mut changed_config = original_config.clone();
        changed_config.store_path = dir.path().join("different-records.jsonl");

        // Prevent creating temporary file needed for writing
        let original_permissions = fs::metadata(dir.path()).unwrap().permissions();
        fs::set_permissions(dir.path(), Permissions::from_mode(0o500)).unwrap();

        // Aborte if user has permissions to write into a R-only dir
        if tempfile::NamedTempFile::new_in(dir.path()).is_ok() {
            fs::set_permissions(dir.path(), original_permissions).unwrap();
            eprintln!("Skipping: this user can write to a read-only directory");

            return;
        }

        let result = save(&path, &changed_config);

        fs::set_permissions(dir.path(), original_permissions).unwrap();

        let err = result.unwrap_err();
        assert!(
            matches!(err, ConfigError::Write { path: failed_path, source } if failed_path ==path && source.kind() == ErrorKind::PermissionDenied)
        );

        assert_eq!(fs::read(&path).unwrap(), original_bytes);

        let reopened = load_from(&path).unwrap();
        assert_eq!(reopened.store_path, original_config.store_path);
    }
}
