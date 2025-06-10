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
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.gh
            pkgs.pre-commit
          ];

          RUST_LOG = "info";

          shellHook = ''
            echo "Gate development environment"
            echo "Rust version: $(rustc --version)"
            echo "RUST_LOG set to: $RUST_LOG"
          '';
        };
      });
}
