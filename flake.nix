{
  description = "Gate - P2P AI Compute Network";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        gatePackage = pkgs.rustPlatform.buildRustPackage {
          pname = "hellas-gate-cli";
          version = "0.1.0";
          
          src = pkgs.lib.cleanSource ./.;
          
          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          buildInputs = with pkgs; [
            openssl
          ];
          
          nativeBuildInputs = with pkgs; [
            pkg-config
            protobuf
            rustToolchain
          ];

          cargoBuildFlags = [ "--package" "gate" ];
        };
      in
      {
        packages = {
          default = gatePackage;
          gate = gatePackage;
        };

        checks = {
          gate = gatePackage;
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            openssl
            pkg-config
            gh
            pre-commit
            cargo-expand
            cargo-udeps
            cargo-outdated
            protobuf
          ];

          RUST_LOG = "info";

          shellHook = ''
            # Find git repository root and set GATE_STATE_DIR
            REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
            export GATE_STATE_DIR="$REPO_ROOT/.state"

            echo "Gate development environment"
            echo "Rust version: $(rustc --version)"
            echo "RUST_LOG set to: $RUST_LOG"
            echo "GATE_STATE_DIR set to: $GATE_STATE_DIR"
          '';
        };
      });
}
