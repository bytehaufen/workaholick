{
  rustPlatform,
  version ? "git",
  rev ? "unknown",
  date ? "19700101",
  lib,
}: let
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../src
    ];
  };
in
  rustPlatform.buildRustPackage (finalAttrs: {
    pname = "workaholick";
    inherit version src;

    cargoLock = {
      lockFile = "${src}/Cargo.lock";
    };

    cargoHash = lib.fakeHash;

    env = {
      VERGEN_GIT_SHA = rev;
      VERGEN_BUILD_DATE = builtins.concatStringsSep "-" (builtins.match "(.{4})(.{2})(.{2}).*" date);
    };

    postInstall = '''';

    meta = {
      description = "todo";
      homepage = "https://github.com/bytehaufen/workaholick";
      license = lib.licenses.mit;
      mainProgram = "workaholick";
    };
  })
