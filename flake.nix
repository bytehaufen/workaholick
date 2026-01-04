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
    self,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [(import rust-overlay)];
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = ["rust-src" "rustfmt" "clippy"];
        };

        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };
        rev = self.shortRev or self.dirtyShortRev or "dirty";
        date = self.lastModifiedDate or self.lastModified or "19700101";
        version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package.version + "pre${builtins.substring 0 8 date}_${rev}";

        preCommit = pre-commit-hooks.lib.${system}.run {
          src = ./.;
          excludes = [".lock" ".json"];

          hooks = {
            alejandra.enable = true;

            rustfmt.enable = true;

            typos = {
              enable = true;
              settings = {
                write = true;
                configPath = "./.typos.toml";
              };
            };

            markdownlint.enable = true;
          };
        };
      in {
        checks.pre-commit-check = preCommit;

        formatter = pkgs.alejandra;

        devShells = {
          default = pkgs.callPackage ./nix/shell.nix {
            inherit rustToolchain preCommit;
            inherit (self.packages.${system}) workaholick;
          };
        };

        packages = {
          workaholick = pkgs.callPackage ./nix/workaholick.nix {
            inherit version rev date rustPlatform;
          };
        };
      }
    );
}
