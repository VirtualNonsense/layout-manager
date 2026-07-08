{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Rust
            rustToolchain
            tombi
            nil

            # ESP Tools
            espflash # flashen & monitor
            espup # toolchain manager (optional)

            # Build Tools
            gnumake
            cmake
            ninja
            pkg-config

            # Debugging
            probe-rs-tools

            # Nützliches
            cargo-generate # templates
            cargo-watch
          ];

          shellHook = ''
            echo "🦀 ESP32-C3 Rust Shell ready"
          '';
        };
      }
    );
}
