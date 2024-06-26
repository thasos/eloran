{
  description = "eloran";
  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        overrides = (builtins.fromTOML (builtins.readFile ./rust-toolchain.toml));
      in
      {
        devShell = pkgs.mkShell {
          buildInputs = [
            pkgs.rustup pkgs.cargo pkgs.rustc pkgs.gcc # rust dev base
            pkgs.rust-analyzer pkgs.clippy # rust code quality
            pkgs.clang # C compiler
            pkgs.cargo-insta # snapshot testing
            # eloran specific
            pkgs.grass-sass # scss compiler
            pkgs.pkg-config pkgs.glib # needed for linking C libraries (cairo, poppler, libarchive)
            pkgs.cairo # PDF rendering
            pkgs.poppler # needed by Cairo
            pkgs.libarchive # needed by the final binary
            pkgs.sqlite # client for database debugging
          ];
          RUSTC_VERSION = overrides.toolchain.channel;
          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
          shellHook = ''
            export PATH=$PATH:''${CARGO_HOME:-~/.cargo}/bin
            export PATH=$PATH:''${RUSTUP_HOME:-~/.rustup}/toolchains/$RUSTC_VERSION-x86_64-unknown-linux-gnu/bin/
            export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="clang"
          '';
        };
      });
}
