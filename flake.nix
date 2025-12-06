{
  description = "workaholick - Rust nightly dev environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = {
    nixpkgs,
    flake-utils,
    rust-overlay,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [(import rust-overlay)];
        };

        rustToolchain = pkgs.rust-bin.nightly.latest.default;
      in {
        devShells.default = pkgs.mkShell {
          name = "workaholick-dev-shell";

          buildInputs = [
            rustToolchain
            pkgs.rust-analyzer
            pkgs.pkg-config
            pkgs.openssl
            pkgs.cmake
            pkgs.gcc
          ];

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        };
      }
    );
}
