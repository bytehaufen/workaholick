use anyhow::{Result, anyhow};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorCommand {
    pub program: String,
    pub args: Vec<String>,
}

pub fn choose_editor(env_visual: Option<String>, env_editor: Option<String>) -> String {
    env_visual
        .filter(|s| !s.trim().is_empty())
        .or(env_editor.filter(|s| !s.trim().is_empty()))
        .unwrap_or("vi".into())
}

pub fn parse_editor_command(cmd: &str) -> Result<EditorCommand> {
    let parts = shlex::split(cmd).ok_or_else(|| anyhow!("Could not parse Editor: {cmd}"))?;
    if parts.is_empty() {
        return Ok(EditorCommand {
            program: "vi".into(),
            args: vec![],
        });
    }

    Ok(EditorCommand {
        program: parts[0].clone(),
        args: parts[1..].to_vec(),
    })
}
