{
  pkgs,
  mkShell,
  workaholick,
  rustToolchain,
  preCommit,
}:
mkShell {
  packages = with pkgs;
    [
      (rustToolchain.override {
        extensions = ["rust-src" "rustfmt" "rust-analyzer" "clippy"];
      })
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

  inputsFrom = [workaholick];

  env.RUST_BACKTRACE = "1";

  RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
}
