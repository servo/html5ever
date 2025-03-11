# please read flake introduction here:
# https://fasterthanli.me/series/building-a-rust-service-with-nix/part-10#a-flake-with-a-dev-shell
{
  description = "The fairsync importer prototype flake";
  inputs = {
    rust-overlay.url = "github:oxalica/rust-overlay";
  };
  outputs =
  { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs {
            inherit system overlays;
          };
          rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          platform_packages =
            if pkgs.stdenv.isLinux then
              with pkgs; [ ]
            else if pkgs.stdenv.isDarwin then
              with pkgs.darwin.apple_sdk.frameworks; [
                CoreFoundation
                Security
                SystemConfiguration
              ]
            else
              throw "unsupported platform";
        in
        with pkgs;
        rec {
          trunk = pkgs.callPackage ./trunk.nix {
            inherit (darwin.apple_sdk.frameworks) CoreServices Security SystemConfiguration;
          };
          #leptosfmt = pkgs.callPackage ./leptosfmt.nix {};

          devShells.default = mkShell {
            buildInputs = [
              rust
              wasm-pack
              firefox
              trunk                    # required to bundle the frontend
              binaryen                 # required to minify WASM files with wasm-opt
              git
              pkg-config
              just                     # task runner
              #nodejs                   # required to install tailwind plugins
            ];
          };
        }
      );
}
