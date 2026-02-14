# List all available just recipes
default:
  @just --list

# Build Rust documentation and open it in the browser
doc:
  cargo doc --no-deps
  xdg-open http://0.0.0.0:8000/workaholick/index.html
  simple-http-server target/doc

# Format all Nix and Rust source files
fmt:
  nix fmt .
  :
  cargo fmt

# Run all Nix flake checks defined in `flake.nix`
check:
  nix flake check -L

# Build the project using Nix
build:
  nix build "#workaholick" -L

# Run all Rust tests (accepts extra arguments)
test *ARGS:
  cargo test -- --nocapture {{ARGS}}

# Print Rust test coverage to stdout
cov *ARGS:
  cargo llvm-cov {{ARGS}}

# Generate HTML test coverage report
cov-html *ARGS:
  cargo llvm-cov {{ARGS}} --html

# Generate HTML test coverage report and open it in the browser
cov-html-open *ARGS:
  @just cov-html {{ARGS}}
  xdg-open target/llvm-cov/html/index.html
