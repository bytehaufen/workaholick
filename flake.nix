{
  description = "Track work, break, and daily activity from the terminal";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    pre-commit-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    nixpkgs,
    flake-utils,
    rust-overlay,
    crane,
    pre-commit-hooks,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [(import rust-overlay)];
        };

        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        inherit (cargoToml.package) version;

        src = craneLib.cleanCargoSource ./.;

        commonArgs = {
          pname = "workaholick";
          inherit version src;

          strictDeps = true;

          meta = {
            inherit (cargoToml.package) description;

            homepage = "https://github.com/bytehaufen/workaholick";
            license = pkgs.lib.licenses.mit;
            mainProgram = "workaholick";
          };
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        workaholick = craneLib.buildPackage (commonArgs
          // {
            inherit cargoArtifacts;

            nativeBuildInputs = [
              pkgs.installShellFiles
            ];

            # Tests are handled by checks.test.
            doCheck = false;

            postInstall = ''
              installShellCompletion \
                --cmd workaholick \
                --bash <($out/bin/workaholick completions bash) \
                --fish <($out/bin/workaholick completions fish) \
                --zsh <($out/bin/workaholick completions zsh)
            '';
          });

        preCommit = pre-commit-hooks.lib.${system}.run {
          src = ./.;
          settings.rust.check.cargoDeps = pkgs.rustPlatform.importCargoLock {
            lockFile = ./Cargo.lock;
          };
          excludes = [
            "\\.lock$"
            "\\.json$"
          ];

          hooks = {
            alejandra.enable = true;
            rustfmt.enable = true;
            markdownlint.enable = true;

            clippy = {
              enable = true;

              packageOverrides = {
                cargo = rustToolchain;
                clippy = rustToolchain;
              };

              settings = {
                allFeatures = true;
                denyWarnings = true;
                extraArgs = "--workspace --all-targets";
              };

              pass_filenames = false;
            };

            typos = {
              enable = true;
              settings.configPath = "./.typos.toml";
            };
          };
        };
      in {
        formatter = pkgs.alejandra;

        packages = {
          inherit workaholick;
          default = workaholick;
        };

        apps.default = {
          type = "app";
          program = "${pkgs.lib.getExe workaholick}";
          meta =
            workaholick.meta
            // {
              description = workaholick.meta.description or "workaholick CLI";
            };
        };

        checks = {
          inherit workaholick;

          pre-commit-check = preCommit;
          fmt = craneLib.cargoFmt commonArgs;

          clippy = craneLib.cargoClippy (commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--workspace --all-targets --all-features -- -D warnings";
            });

          test = craneLib.cargoTest (commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--workspace --all-features";
            });
        };

        devShells = let
          inherit (pkgs.llvmPackages) llvm;
        in {
          default = pkgs.mkShell {
            inherit (preCommit) shellHook;

            inputsFrom = [workaholick];

            packages = with pkgs;
              [
                rustToolchain
                just
                cargo-watch
                cargo-llvm-cov
                llvm
                pre-commit
              ]
              ++ preCommit.enabledPackages;

            env = {
              RUST_BACKTRACE = "1";
              RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
              LLVM_COV = "${llvm}/bin/llvm-cov";
              LLVM_PROFDATA = "${llvm}/bin/llvm-profdata";
              RUST_LOG = "workaholick=debug";

              LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath (with pkgs; [
                libclang
                libgtkflow4
                libxcb-render-util
                libxcb
                libxkbcommon
                libx11
                libxcursor
                libxrandr
                libxi
                libxinerama
                wayland
                vulkan-loader
                libGL
              ])}:$LD_LIBRARY_PATH";
            };
          };
        };
      }
    );
}
