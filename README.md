# Workaholick 👅

<!-- markdownlint-disable MD013 MD033 -->

Highly opinionated personal time-tracking CLI, written in safe Rust.

Track work blocks, breaks, vacation, and sick days. See how your week compares with your schedule.

This is my first Rust project, and I use it as my daily driver. Still experimental.
It tracks the work. Doing the work remains your problem.

I use it on Linux with Nix. Other platforms are untested.

## Installation

### Cargo

Install Rust through [rustup](https://rustup.rs/), then clone the repository and install the app:

```bash
git clone https://github.com/bytehaufen/workaholick.git
cd workaholick
cargo install --path .
workaholick --help
```

Make sure `~/.cargo/bin` is on your `PATH`.

Just trying it? From the checkout, use `cargo run -- --help` instead of installing.
For the walkthrough, replace `workaholick` with `cargo run --`.

### Nix

You need Nix with flakes and `nix-command` enabled.

```bash
nix run github:bytehaufen/workaholick -- --help

# Install it into your user profile, so "workaholick" is on PATH
nix profile add github:bytehaufen/workaholick
```

The examples below assume `workaholick` is on your `PATH`.

## Walkthrough

Add `--help` to any command when the examples stop being helpful.

### A working day

```bash
# Create initial configuration; refuses to overwrite an existing file
workaholick config init

# Set your schedule and data location
workaholick config edit

# Start working in the office now
workaholick work start office

# Finish a block of work and start the next one
workaholick work log "#341" "Fix a bug"

# Coffee is not a work context
workaholick work break-start
workaholick work break-end

# Finish work and describe the last block
workaholick work end "#525" "Implement feature A"

# Show records for the latest recorded date
workaholick day

# Show completed work blocks for the current week, then last week's
workaholick activity
workaholick activity week -1

# Show this week's timesheet, then last week's
workaholick timesheet week
workaholick timesheet week -1
```

Work contexts are `home`, `office`, and `travel`. For `work log` and `work end`,
the first argument is the title; the optional second argument is the description.

### Days off

Full-day absences need dates without work records. Half-day absences can share a
date with work records:

```bash
workaholick absence vacation --from 2026-12-24 --to 2026-12-25
workaholick absence sick --date 2026-12-28 --portion half-day --note "doctor - lobotomy"
```

The date range includes both ends. Without a date, an absence is recorded for
today. The default portion is a full day.

### Editing records

Commands print record IDs. Use those to edit or delete a record:

```bash
# Replace 1 with the ID you actually want
workaholick edit 1
workaholick delete 1
```

The `delete` command accepts one ID at a time. It may reject a deletion that would
leave an invalid work sequence. There is **NO** undo.

Editing uses `$VISUAL`, then `$EDITOR`, then `vi`. For a GUI editor, use its wait
option, for example `VISUAL="code --wait" workaholick edit 1`.
Record edits use RON; config edits use TOML. Keep the structure and change the values.

## Configuration

On Linux, the default config is `~/.config/workaholick/config.toml`, or
`$XDG_CONFIG_HOME/workaholick/config.toml` when that variable is set.
`workaholick config show` prints the file.

Example configuration, with your home directory instead of `/home/user`:

```toml
store_path = "/home/user/workaholick/data.jsonl"

# Target minutes per day
[schedule]
mon_minutes = 480
tue_minutes = 480
wed_minutes = 480
thu_minutes = 480
fri_minutes = 480
sat_minutes = 0
sun_minutes = 0

# The only formatter for now
[cli]
formatter = "markdown"

# Rounding settings applied for output like
# `workaholick timesheet`
[rounding]
interval_minutes = 5
start = "ceil"
break_start = "floor"
break_end = "ceil"
end = "floor"
```

A relative `store_path` is resolved from the directory you run the command in,
not the config file's directory. An absolute path avoids surprises.

The `[rounding]` section adjusts work and break timestamps for timesheet calculations
without changing stored records. Each boundary supports `ceil` (up), `floor` (down),
or `none` (the default), using `interval_minutes` as the rounding interval.

To try things without touching your real records, choose an unused config path:

```bash
workaholick --config /tmp/workaholick-demo.toml config init
workaholick --config /tmp/workaholick-demo.toml config edit
```

In that config, set `store_path = "/tmp/workaholick-demo-data.jsonl"` (use an unused
path). Then put `--config /tmp/workaholick-demo.toml` before every subcommand.

## Things worth knowing

- `day` without a date shows the latest recorded date, not necessarily today.
- This week's report only includes time up to now. Future absences are not included yet.
- Absence credit comes from your schedule: a full day on an eight-hour workday
  credits eight hours; on a zero-hour day, it credits zero. It is not worked time.
- Work stays open until you end it, including across midnight.

## Backups and small disasters

The JSONL file is the data store, not an append-only history. Edits and deletes
rewrite it. Make sure no command or editor is still running, then copy the file
somewhere safe:

```bash
# Default location; adjust this if you changed store_path
cp ~/workaholick/data.jsonl ~/workaholick/data.jsonl.bak
```

Back up the config too if you want the same schedule when restoring.
If records are malformed, repair a copy first. Invalid data can prevent even
`edit` and `delete` from starting.

A crash can leave `data.jsonl.lock` beside the records file. If the app says the
store is locked, first check that no instance or editor is still running.
Then back up the records and remove only that store's stale lock file.

> I keep `data.jsonl` in a directory synced with Syncthing so I can access it on
> all my devices.

## Development

### Rust binary

From a checkout, build an optimized binary and run it:

```bash
cargo build --release
./target/release/workaholick --help
```

For local checks without Nix or `just`:

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

### Nix package

From a checkout:

```bash
# Enter the development environment
nix develop

# List available just recipes
just -l

# Build the Nix package
just build
./result/bin/workaholick --help
```

## Shell completions

Only the zsh and bash completions have been tested. If another shell gets upset,
feel free to open an issue.

The Nix package includes bash, fish, and zsh completions.

<details>
<summary>Manual installation</summary>

```sh
# Bash
mkdir -p ~/.local/share/bash-completion/completions
workaholick completions bash > ~/.local/share/bash-completion/completions/workaholick

# Fish
mkdir -p ~/.config/fish/completions
workaholick completions fish > ~/.config/fish/completions/workaholick.fish

# Zsh
mkdir -p ~/.zfunc
workaholick completions zsh > ~/.zfunc/_workaholick

# Elvish
mkdir -p ~/.config/elvish/lib ~/.config/elvish
workaholick completions elvish > ~/.config/elvish/lib/workaholick.elv
printf 'use workaholick\n' >> ~/.config/elvish/rc.elv
```

```powershell
# Powershell
if (!(Test-Path -Path $PROFILE)) { New-Item -ItemType File -Path $PROFILE -Force }
$Completion = Join-Path (Split-Path $PROFILE) 'workaholick-completions.ps1'
workaholick completions powershell | Out-File -Encoding utf8 $Completion
Add-Content -Path $PROFILE ". $Completion"
```

For zsh, make sure `~/.zfunc` is in `fpath` before `compinit` runs.

</details>

## Roadmap

- [ ] Android frontend

## Contributing

Found a bug? Open an issue. For bigger changes, ask first.

## License

MIT. See [LICENSE](LICENSE).
