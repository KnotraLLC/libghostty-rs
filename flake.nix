{
  description = "A very basic rust flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs @ {
    self,
    nixpkgs,
    crane,
    rust-overlay,
    ...
  }: let
    allSystems = ["x86_64-linux" "aarch64-linux" "aarch64-darwin"];
    forAllSystems = f:
      nixpkgs.lib.genAttrs allSystems (system:
        f {
          pkgs = import nixpkgs {
            inherit system;
            overlays = [(import rust-overlay)];
          };
          inherit system;
        });
  in {
    packages = forAllSystems ({pkgs, system}: let
      craneLib = (crane.mkLib pkgs).overrideToolchain (p:
        p.rust-bin.stable.latest.default.override {
          extensions = ["rust-src" "rust-std" "clippy" "rustfmt" "rust-analyzer"];
        });
      unfilteredRoot = ./.;

      src = pkgs.lib.fileset.toSource {
        root = unfilteredRoot;
        fileset = pkgs.lib.fileset.unions [
          (craneLib.fileset.commonCargoSources unfilteredRoot)
        ];
      };

      commonArgs = {
        inherit src;
        strictDeps = true;

        nativeBuildInputs = [
          pkgs.pkg-config
        ];

        buildInputs =
          [
            pkgs.openssl
            pkgs.libclang
          ]
          ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
            pkgs.darwin.apple_sdk.frameworks.Security
          ];
      };

      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      application = craneLib.buildPackage (
        commonArgs
        // {
          inherit cargoArtifacts;

          nativeBuildInputs =
            (commonArgs.nativeBuildInputs or [])
            ++ [
            ];
        }
      );
    in {
      default = application;
    });

    devShells = forAllSystems ({pkgs, system}: let
      craneLib = (crane.mkLib pkgs).overrideToolchain (p:
        p.rust-bin.stable.latest.default.override {
          extensions = ["rust-src" "rust-std" "clippy" "rustfmt" "rust-analyzer"];
        });
    in {
      default = craneLib.devShell {
        buildInputs = [];
        packages = with pkgs; [];
      };
    });
  };
}
