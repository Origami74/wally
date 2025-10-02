{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  inputs.crane.url = "github:ipetkov/crane";
  inputs.fenix = {
    url = "github:nix-community/fenix";
    inputs.nixpkgs.follows = "nixpkgs";
    inputs.rust-analyzer-src.follows = "";
  };
  inputs.flake-utils.url = "github:numtide/flake-utils";
  inputs.advisory-db = {
    url = "github:rustsec/advisory-db";
    flake = false;
  };

  outputs =
    {
      self,
      crane,
      fenix,
      flake-utils,
      nixpkgs,
      advisory-db,
      ...
    }:

    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          config.allowBroken = true;
        };

        inherit (pkgs) lib;

        craneLib = crane.mkLib pkgs;
        src = craneLib.cleanCargoSource ./src-tauri/.;

        commonArgs = {
          inherit src;
          strictDeps = true;

          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [
            pkgs.sqlite
            pkgs.pnpm
            pkgs.nodejs_22
            pkgs.openssl
            pkgs.protobuf
          ]
          ++ lib.optionals pkgs.stdenv.isLinux [
            pkgs.cairo
            pkgs.cargo-tauri
            pkgs.pango
            pkgs.atkmm
            pkgs.at-spi2-atk
            pkgs.gdk-pixbuf
            pkgs.glib
            pkgs.gtk3
            pkgs.harfbuzz
            pkgs.librsvg
            pkgs.libsoup_3
            pkgs.webkitgtk_4_1
          ]
          ++ lib.optionals pkgs.stdenv.isDarwin [
            # Additional darwin specific inputs can be set here
            pkgs.libiconv
            pkgs.cargo-tauri
          ];
          PROTOC = "${pkgs.protobuf}/bin/protoc";
          PROTOC_INCLUDE = "${pkgs.protobuf}/include";

        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        my-crate = craneLib.buildPackage (
          commonArgs
          // {
            cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          }
        );
      in
      {
        checks = {
          inherit my-crate;

          # Cargo clippy
          my-workspace-clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );
          # Check formatting
          my-workspace-fmt = craneLib.cargoFmt {
            inherit src;
          };
          # Audit dependencies
          my-workspace-audit = craneLib.cargoAudit {
            inherit src advisory-db;
          };
        };
        packages.default = my-crate;

        apps.default = flake-utils.lib.mkApp {
          drv = my-crate;
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};
          packages = [
            pkgs.cargo-watch
            pkgs.cargo-outdated
          ];
        };

        formatter = pkgs.nixpkgs-fmt;
      }
    );
}
