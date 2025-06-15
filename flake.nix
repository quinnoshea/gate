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
          version = "0.0.2";
          
          src = pkgs.lib.cleanSource ./.;
          
          doCheck = false;
          
          cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes = {
              "instant-acme-0.8.0" = "sha256-UF/nJ8Nxvxk2F6U689Pkv18kpfNnwBDsOnmaX9wFBCU=";
            };
          };

          postPatch = ''
            # Override specific problematic Cargo.toml files in vendored dependencies
            # We'll patch the source after Nix extracts and vendors them
            echo "Preparing to patch workspace lints issues..."
          '';
          
          preBuild = ''
            # Fix workspace lints inheritance issue in vendored dependencies
            echo "Fixing workspace lints in vendored dependencies..."
            if [ -d cargo-vendor-dir ]; then
              echo "Found cargo-vendor-dir, looking for iroh..."
              ls -la cargo-vendor-dir/ | grep iroh
              
              # First, let's find the iroh Cargo.toml and examine it
              IROH_TOML=$(find cargo-vendor-dir -name "Cargo.toml" -path "*/iroh-*" | head -1)
              if [ -n "$IROH_TOML" ]; then
                echo "Found iroh Cargo.toml at: $IROH_TOML"
                echo "Content before fix:"
                grep -A 5 -B 5 "lints" "$IROH_TOML" || echo "No lints section found initially"
                
                # Remove lints references
                sed -i.bak '/workspace\.lints/d' "$IROH_TOML"
                sed -i.bak '/\[lints\]/,/^$/d' "$IROH_TOML"
                
                echo "Content after fix:"
                grep -A 5 -B 5 "lints" "$IROH_TOML" || echo "Lints section successfully removed"
              else
                echo "No iroh Cargo.toml found, listing all toml files:"
                find cargo-vendor-dir -name "Cargo.toml" | head -10
              fi
            else
              echo "cargo-vendor-dir not found"
            fi
          '';

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
            cargo-machete
            protobuf
          ];

          RUST_LOG = "info";
          OTLP_ENDPOINT = "https://jaeger.internal.lsd-ag.ch/v1/traces";

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
