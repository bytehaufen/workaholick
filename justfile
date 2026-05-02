set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# List all available just recipes
default:
    @just --list

# Format Nix and Rust files
fmt:
    nix fmt .
    cargo fmt --all

# Check Rust formatting only
fmt-check:
    cargo fmt --all -- --check

# Alias for local CI parity
ci: nix-check build

# Run through Cargo
run *ARGS:
    cargo run -- {{ ARGS }}

# Run through Nix app
nix-run *ARGS:
    nix run . -- {{ ARGS }}

# Fast local check
check:
    cargo check --workspace --all-targets --all-features

# Run all Nix flake checks defined in `flake.nix`
nix-check:
    nix flake check --print-build-logs

# Build the project using Nix
build:
    nix build . --print-build-logs

# Local Clippy check
clippy:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Run all Rust tests
test *ARGS:
    cargo test --workspace --all-targets --all-features  {{ ARGS }} -- --nocapture

# Watch local checks
watch:
    cargo watch -c -x "clippy --workspace --all-targets --all-features"

# Build Rust documentation and open it in the browser
doc:
    cargo doc --open --no-deps

# Print test coverage
cov *ARGS:
    cargo llvm-cov --workspace --all-features {{ ARGS }}

# Generate and open HTML coverage report
cov-html *ARGS:
    cargo llvm-cov --workspace --all-features --html --open {{ ARGS }}

# Run pre-commit hooks on all files
pre-commit:
    pre-commit run --all-files

# Install pre-commit hooks manually
hooks:
    pre-commit install

# Update lock files
update:
    nix flake update
    cargo update

# Remove local build outputs
clean:
    cargo clean
    rm -rf result
