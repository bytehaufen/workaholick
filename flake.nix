{
  description = "workaholick - Rust nightly dev environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";

    pre-commit-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    nixpkgs,
    flake-utils,
    rust-overlay,
    pre-commit-hooks,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [(import rust-overlay)];
        };

        rustToolchain = pkgs.rust-bin.nightly.latest.default.override {
          extensions = ["rust-src" "rustfmt" "clippy"];
        };

        preCommit = pre-commit-hooks.lib.${system}.run {
          src = ./.;
          excludes = [".lock" ".json"];

          hooks = {
            alejandra.enable = true;

            clippy = {
              enable = true;
              package = rustToolchain;
              settings.allFeatures = true;
            };

            rustfmt.enable = true;

            typos = {
              enable = true;
              settings = {
                write = true;
                configPath = "./.typos.toml";
              };
            };
          };
        };
      in {
        checks.pre-commit-check = preCommit;

        devShells.default = pkgs.mkShell {
          name = "workaholick-dev-shell";

          inherit (preCommit) shellHook enabledPackages;

          buildInputs = with pkgs;
            [
              rustToolchain
              rust-analyzer
              pkg-config
              openssl
              cmake
              gcc
              just
              cargo-watch
              pre-commit
              simple-http-server
            ]
            ++ preCommit.enabledPackages;

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        };
      }
    );
}
