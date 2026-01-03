use std::{
    fs::{self, Permissions},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use directories::UserDirs;
use serde::{Deserialize, Serialize};

use crate::editor::{Editor, system::FileType};

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const CONFIG_NAME: &str = "config";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub store_path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            store_path: default_store_path(),
        }
    }
}

pub fn init() -> Result<PathBuf> {
    let path = config_path()?;

    if path.exists() {
        anyhow::bail!("Config already exists at {} --aborting", path.display())
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating config directory {}", parent.display()))?;
    }

    let cfg = Config::default();
    save(&path, &cfg)?;

    Ok(path)
}

pub fn load() -> Result<Config> {
    let path = existing_config_path()?;
    confy::load_path(&path).with_context(|| format!("reading config file {}", path.display()))
}

pub fn show() -> Result<String> {
    let path = existing_config_path()?;
    fs::read_to_string(&path).with_context(|| format!("reading config file {}", path.display()))
}

pub fn edit(editor: &dyn Editor) -> Result<()> {
    let path = config_path()?;
    let original = fs::read_to_string(&path)
        .with_context(|| format!("reading config file {}", path.display()))?;

    let header = format!(
        "# {APP_NAME} config\n\
         # Save and close the editor to apply changes.\n\
         # Keep the format valid.\n\n"
    );

    let editored = editor
        .edit(&(header + &original), FileType::Toml)
        .context("opening editor")?;
    let cfg: Config = toml::from_str(&editored).context("config TOML is invalid (not saving)")?;
    save(&path, &cfg)?;

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

fn default_store_path() -> PathBuf {
    let home = UserDirs::new()
        .map(|user_dirs| user_dirs.home_dir().to_path_buf())
        .expect("no home directory found");

    home.join(APP_NAME).join("data.jsonl")
}

#[cfg(test)]
mod tests {
    use crate::config::APP_NAME;

    #[test]
    fn app_name_is_set() {
        assert_eq!(APP_NAME, "workaholick")
    }
}
