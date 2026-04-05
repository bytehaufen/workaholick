# Workaholick 👅

<!-- markdownlint-disable MD013 -->

## Configuration

- Path to data
- `weekly_target_hours` for weekly summary balance, e.g. `40.0` or `37.5`
- Output format 'markdown' \| 'plain'

## Shell completions

Non-NixOS install:

```sh
# bash
mkdir -p ~/.local/share/bash-completion/completions
workaholick completions bash > ~/.local/share/bash-completion/completions/workaholick

# fish
mkdir -p ~/.config/fish/completions
workaholick completions fish > ~/.config/fish/completions/workaholick.fish

# zsh
mkdir -p ~/.zfunc
workaholick completions zsh > ~/.zfunc/_workaholick

# elvish
mkdir -p ~/.config/elvish/lib ~/.config/elvish
workaholick completions elvish > ~/.config/elvish/lib/workaholick.elv
printf 'use workaholick\n' >> ~/.config/elvish/rc.elv
```

```powershell
# powershell
if (!(Test-Path -Path $PROFILE)) { New-Item -ItemType File -Path $PROFILE -Force }
$Completion = Join-Path (Split-Path $PROFILE) 'workaholick-completions.ps1'
workaholick completions powershell | Out-File -Encoding utf8 $Completion
Add-Content -Path $PROFILE ". $Completion"
```

For zsh, make sure `~/.zfunc` is in `fpath`. Nix package builds install bash, fish, and zsh completions automatically.

## TODO

- Error handling
- Test more
- Doc code
- Add README description

### Features

- Webfrontend
