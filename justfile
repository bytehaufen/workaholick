doc:
  cargo doc --no-deps
  xdg-open http://0.0.0.0:8000/workaholick/index.html
  simple-http-server target/doc

fmt:
  nix fmt .
  :
  cargo fmt

check:
  nix flake check -L

build:
  nix build "#workaholick" -L
