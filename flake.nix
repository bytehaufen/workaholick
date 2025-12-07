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

        rustToolchain = pkgs.rust-bin.nightly.latest.default;

        preCommit = pre-commit-hooks.lib.${system}.run {
          src = ./.;
          excludes = [".lock" ".json"];

          hooks = {
            alejandra.enable = true;

            clippy = {
              enable = true;
              packageOverrides.cargo = pkgs.cargo;
              packageOverrides.clippy = pkgs.clippy;
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

          buildInputs =
            [
              rustToolchain
              pkgs.rust-analyzer
              pkgs.pkg-config
              pkgs.openssl
              pkgs.cmake
              pkgs.gcc
              pkgs.just
              pkgs.cargo-watch
              pkgs.pre-commit
            ]
            ++ preCommit.enabledPackages;

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        };
      }
    );
}
