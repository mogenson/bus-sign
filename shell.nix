{ pkgs ? import <nixpkgs> { } }:
let
  overrides = (builtins.fromTOML (builtins.readFile ./rust-toolchain.toml));
in
pkgs.mkShell {
  buildInputs = with pkgs; [
    elf2uf2-rs
    rustup
    git
  ];

  RUSTC_VERSION = overrides.toolchain.channel;

  shellHook = ''
    export PATH=$PATH:''${CARGO_HOME:-~/.cargo}/bin
  '';
}
