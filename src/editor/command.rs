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
        return Ok(EditorCommand { program: "vi".into(), args: vec![] });
    }

    Ok(EditorCommand { program: parts[0].clone(), args: parts[1..].to_vec() })
}

#[cfg(test)]
mod tests {
    use crate::editor::command::{choose_editor, parse_editor_command};

    #[test]
    fn choose_editor_chooses_visual_editor() {
        let chosen = choose_editor(Some("visual env".into()), Some("editor env".into()));
        assert_eq!(chosen, "visual env");

        let chosen = choose_editor(Some("visual env".into()), None);
        assert_eq!(chosen, "visual env");
    }

    #[test]
    fn choose_editor_chooses_editor_editor() {
        let chosen = choose_editor(None, Some("editor env".into()));
        assert_eq!(chosen, "editor env");
    }

    #[test]
    fn choose_editor_chooses_fallback_editor() {
        let chosen = choose_editor(None, None);
        assert_eq!(chosen, "vi");
    }

    #[test]
    fn choose_editor_ignores_blank_visual_and_uses_editor() {
        let chosen = choose_editor(Some("   ".into()), Some("editor env".into()));
        assert_eq!(chosen, "editor env");
    }

    #[test]
    fn choose_editor_ignores_blank_visual_and_editor_and_uses_fallback() {
        let chosen = choose_editor(Some("   ".into()), Some("\n\t".into()));
        assert_eq!(chosen, "vi");
    }

    #[test]
    fn parse_editor_parses_empty_as_default() {
        let parsed = parse_editor_command("").unwrap();
        assert_eq!(parsed.program, "vi");
        assert!(parsed.args.is_empty());

        let parsed = parse_editor_command("   ").unwrap();
        assert_eq!(parsed.program, "vi");
        assert!(parsed.args.is_empty());
    }

    #[test]
    fn parse_editor_parses_single_program() {
        let parsed = parse_editor_command("nvim").unwrap();
        assert_eq!(parsed.program, "nvim");
        assert!(parsed.args.is_empty());
    }

    #[test]
    fn parse_editor_parses_multi_args() {
        let parsed = parse_editor_command("nvim +42 file.txt").unwrap();
        assert_eq!(parsed.program, "nvim");
        assert_eq!(parsed.args, vec!["+42".to_string(), "file.txt".to_string()]);
    }

    #[test]
    fn parse_editor_preserves_quoted_arg() {
        let parsed = parse_editor_command("code --goto 'src/main.rs:10:2'").unwrap();
        assert_eq!(parsed.program, "code");
        assert_eq!(parsed.args, vec!["--goto".to_string(), "src/main.rs:10:2".to_string()]);
    }

    #[test]
    fn parse_editor_rejects_invalid_shell_syntax() {
        let err = parse_editor_command("nvim 'unterminated").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Could not parse Editor"));
    }
}
