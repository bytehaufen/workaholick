{
  pkgs,
  mkShell,
  workaholick,
  rustToolchain,
  preCommit,
}: let
  inherit (pkgs.llvmPackages) llvm;
in
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
        cargo-llvm-cov
        llvm
        pre-commit
        simple-http-server
      ]
      ++ preCommit.enabledPackages;

    inputsFrom = [workaholick];

    env.RUST_BACKTRACE = "1";

    RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
    LLVM_COV = "${llvm}/bin/llvm-cov";
    LLVM_PROFDATA = "${llvm}/bin/llvm-profdata";
  }
