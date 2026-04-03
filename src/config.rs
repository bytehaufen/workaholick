use std::{
    fs::{self, Permissions},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use directories::UserDirs;
use serde::{Deserialize, Deserializer, Serialize};

use crate::editor::{Editor, ports::FileType};

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const CONFIG_NAME: &str = "config";

#[derive(Debug, Clone, Serialize)]
pub struct Config {
    pub store_path: PathBuf,
    pub weekly_target_hours: f64,
    #[serde(default)]
    pub cli: CliConfig,
}

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct CliConfig {
    #[serde(default)]
    pub formatter: OutputFormatter,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormatter {
    #[default]
    Plain,
    Markdown,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            store_path: default_store_path(),
            weekly_target_hours: default_weekly_target_hours(),
            cli: CliConfig::default(),
        }
    }
}

impl Config {
    pub fn weekly_target_minutes(&self) -> i64 {
        (self.weekly_target_hours * 60.0).round() as i64
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
            weekly_target_hours: Option<f64>,
            #[serde(default)]
            daily_target_hours: Option<f64>,
            #[serde(default)]
            daily_target_minutes: Option<u32>,
            #[serde(default)]
            cli: CliConfig,
        }

        let raw = RawConfig::deserialize(deserializer)?;
        let weekly_target_hours = raw
            .weekly_target_hours
            .or_else(|| raw.daily_target_hours.map(|hours| hours * 5.0))
            .or_else(|| raw.daily_target_minutes.map(|minutes| (f64::from(minutes) / 60.0) * 5.0))
            .unwrap_or_else(default_weekly_target_hours);

        if !weekly_target_hours.is_finite() || weekly_target_hours < 0.0 {
            return Err(serde::de::Error::custom(
                "weekly_target_hours must be a finite non-negative number",
            ));
        }

        Ok(Self { store_path: raw.store_path, weekly_target_hours, cli: raw.cli })
    }
}

pub fn init() -> Result<PathBuf> {
    let path = config_path()?;
    init_at(&path)
}

pub fn load() -> Result<Config> {
    let path = existing_config_path()?;
    load_from(&path)
}

pub fn show() -> Result<String> {
    let path = existing_config_path()?;
    show_from(&path)
}

pub fn edit(editor: &dyn Editor) -> Result<()> {
    let path = config_path()?;
    edit_at(&path, editor)
}

pub fn init_at(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();

    if path.exists() {
        anyhow::bail!("Config already exists at {} --aborting", path.display())
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating config directory {}", parent.display()))?;
    }

    let cfg = Config::default();
    save(path, &cfg)?;

    Ok(path.to_path_buf())
}

pub fn load_from(path: impl AsRef<Path>) -> Result<Config> {
    let path = path.as_ref();
    ensure_exists(path)?;
    confy::load_path(path).with_context(|| format!("reading config file {}", path.display()))
}

pub fn show_from(path: impl AsRef<Path>) -> Result<String> {
    let path = path.as_ref();
    ensure_exists(path)?;
    fs::read_to_string(path).with_context(|| format!("reading config file {}", path.display()))
}

pub fn edit_at(path: impl AsRef<Path>, editor: &dyn Editor) -> Result<()> {
    let path = path.as_ref();
    ensure_exists(path)?;
    let materialized = render_config_for_editing(path)?;

    let header = format!(
        "# {APP_NAME} config\n\
         # Save and close the editor to apply changes.\n\
         # Keep the format valid.\n\n"
    );

    let editored =
        editor.edit(&(header + &materialized), FileType::Toml).context("opening editor")?;
    let cfg: Config = toml::from_str(&editored).context("config TOML is invalid (not saving)")?;
    save(path, &cfg)?;

    Ok(())
}

fn save(path: &Path, cfg: &Config) -> Result<()> {
    confy::store_path_perms(path, cfg, config_file_permissions())
        .with_context(|| format!("writing config file {}", path.display()))
}

fn render_config_for_editing(path: &Path) -> Result<String> {
    let cfg = load_from(path)?;
    toml::to_string_pretty(&cfg)
        .with_context(|| format!("serializing config file {}", path.display()))
}

fn config_file_permissions() -> Permissions {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Permissions::from_mode(0o600)
    }

    #[cfg(not(unix))]
    {
        Permissions::readonly(false)
    }
}

fn config_path() -> Result<PathBuf> {
    confy::get_configuration_file_path(APP_NAME, CONFIG_NAME).context("computing config path")
}

fn existing_config_path() -> Result<PathBuf> {
    let path = config_path()?;

    if !path.exists() {
        anyhow::bail!(
            "Config file does not exist at {} (use `{} config init` to initialize it)",
            path.display(),
            APP_NAME
        );
    }

    Ok(path)
}

fn ensure_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        anyhow::bail!("Config file does not exist at {}", path.display());
    }

    Ok(())
}

fn default_store_path() -> PathBuf {
    let home = UserDirs::new()
        .map(|user_dirs| user_dirs.home_dir().to_path_buf())
        .expect("no home directory found");

    home.join(APP_NAME).join("data.jsonl")
}

fn default_weekly_target_hours() -> f64 {
    40.0
}

#[cfg(test)]
mod tests {
    use crate::config::{APP_NAME, OutputFormatter};
    use crate::editor::ports::{Editor, FileType};
    use anyhow::Result;
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
        fn edit(&self, initial: &str, file_type: FileType) -> Result<String> {
            assert!(matches!(file_type, FileType::Toml));
            self.seen.replace(Some(initial.to_string()));
            Ok(initial.to_string())
        }
    }

    #[test]
    fn init_load_show_custom_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("custom-config.toml");

        super::init_at(&path).unwrap();
        let loaded = super::load_from(&path).unwrap();
        let shown = super::show_from(&path).unwrap();

        assert_eq!(loaded.store_path, super::Config::default().store_path);
        assert_eq!(loaded.weekly_target_hours, 40.0);
        assert_eq!(loaded.cli.formatter, OutputFormatter::Plain);
        assert!(shown.contains("weekly_target_hours"));
        assert!(shown.contains("[cli]"));
        assert!(shown.contains("formatter"));
        assert!(shown.contains("store_path"));
    }

    #[test]
    fn edit_at_materializes_missing_defaults_in_editor_buffer() {
        let dir = tempfile::tempdir().unwrap();
        let path: PathBuf = dir.path().join("partial-config.toml");
        fs::write(&path, "store_path = \"/tmp/work.jsonl\"\n").unwrap();
        let editor = CapturingEditor::new();

        super::edit_at(&path, &editor).unwrap();

        let seen = editor.seen();
        assert!(seen.contains("weekly_target_hours"));
        assert!(seen.contains("[cli]"));
        assert!(seen.contains("formatter = \"plain\""));
        assert!(seen.contains("store_path = \"/tmp/work.jsonl\""));
    }

    #[test]
    fn load_from_accepts_legacy_daily_target_minutes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("legacy-config.toml");
        fs::write(
            &path,
            "store_path = \"/tmp/work.jsonl\"\ndaily_target_minutes = 450\n[cli]\nformatter = \"plain\"\n",
        )
        .unwrap();

        let loaded = super::load_from(&path).unwrap();

        assert_eq!(loaded.weekly_target_hours, 37.5);
        assert_eq!(loaded.weekly_target_minutes(), 2250);
    }

    #[test]
    fn load_from_accepts_fractional_daily_target_hours() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hours-config.toml");
        fs::write(&path, "store_path = \"/tmp/work.jsonl\"\ndaily_target_hours = 7.5\n").unwrap();

        let loaded = super::load_from(&path).unwrap();

        assert_eq!(loaded.weekly_target_hours, 37.5);
        assert_eq!(loaded.weekly_target_minutes(), 2250);
    }

    #[test]
    fn load_from_accepts_fractional_weekly_target_hours() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("weekly-hours-config.toml");
        fs::write(&path, "store_path = \"/tmp/work.jsonl\"\nweekly_target_hours = 32.5\n").unwrap();

        let loaded = super::load_from(&path).unwrap();

        assert_eq!(loaded.weekly_target_hours, 32.5);
        assert_eq!(loaded.weekly_target_minutes(), 1950);
    }

    #[test]
    fn app_name_is_set() {
        assert_eq!(APP_NAME, "workaholick")
    }
}
