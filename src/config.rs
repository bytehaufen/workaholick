use std::{
    fs::{self, Permissions},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use directories::UserDirs;
use serde::{Deserialize, Serialize};

use crate::editor::{Editor, ports::FileType};

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const CONFIG_NAME: &str = "config";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub store_path: PathBuf,
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
        Self { store_path: default_store_path(), cli: CliConfig::default() }
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
    let original = fs::read_to_string(path)
        .with_context(|| format!("reading config file {}", path.display()))?;

    let header = format!(
        "# {APP_NAME} config\n\
         # Save and close the editor to apply changes.\n\
         # Keep the format valid.\n\n"
    );

    let editored = editor.edit(&(header + &original), FileType::Toml).context("opening editor")?;
    let cfg: Config = toml::from_str(&editored).context("config TOML is invalid (not saving)")?;
    save(path, &cfg)?;

    Ok(())
}

fn save(path: &Path, cfg: &Config) -> Result<()> {
    confy::store_path_perms(path, cfg, config_file_permissions())
        .with_context(|| format!("writing config file {}", path.display()))
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

#[cfg(test)]
mod tests {
    use crate::config::{APP_NAME, OutputFormatter};

    #[test]
    fn init_load_show_custom_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("custom-config.toml");

        super::init_at(&path).unwrap();
        let loaded = super::load_from(&path).unwrap();
        let shown = super::show_from(&path).unwrap();

        assert_eq!(loaded.store_path, super::Config::default().store_path);
        assert_eq!(loaded.cli.formatter, OutputFormatter::Plain);
        assert!(shown.contains("[cli]"));
        assert!(shown.contains("formatter"));
        assert!(shown.contains("store_path"));
    }

    #[test]
    fn app_name_is_set() {
        assert_eq!(APP_NAME, "workaholick")
    }
}
