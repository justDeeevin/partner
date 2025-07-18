{
  description = "Build a cargo project";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane.url = "github:ipetkov/crane";

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        inherit (pkgs) lib;

        craneLib = crane.mkLib pkgs;
        src = craneLib.cleanCargoSource ./.;

        runtimeDeps = with pkgs; [
          libclang
          parted
        ];

        commonArgs = {
          inherit src;
          strictDeps = true;
          buildInputs = runtimeDeps;

          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        partner = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
          }
        );
      in
      {
        checks = {
          inherit partner;

          partner-clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );

          partner-fmt = craneLib.cargoFmt {
            inherit src;
          };

          partner-toml-fmt = craneLib.taploFmt {
            src = pkgs.lib.sources.sourceFilesBySuffices src [ ".toml" ];
          };
        };

        packages = {
          default = partner;
        };

        apps.default = flake-utils.lib.mkApp {
          drv = partner;
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};
          LD_LIBRARY_PATH = lib.makeLibraryPath runtimeDeps;
          CPATH = "${pkgs.parted.dev}/include:${pkgs.libcxx.dev}/include/c++/v1:${pkgs.musl.dev}/include";
        };
      }
    );
}
