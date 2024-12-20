# par défaut : lance la 1ère recipe, sinon :
_default:
    just --list --unsorted

run:
    just grass_compile
    cargo run

test:
    just grass_compile
    cargo-insta test --review

review:
    just grass_compile
    cargo-insta review

grass_compile:
    grass --style compressed sass/main.scss css/eloran.css

build:
    just grass_compile
    cargo +nightly build --release -Z build-std=std,panic_abort -Z build-std-features=panic_immediate_abort --target x86_64-unknown-linux-gnu

export PKG_CONFIG_SYSROOT_DIR := "/home/${USER}/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-musl"
build_musl:
    just grass_compile
    RUSTFLAGS='-C target-feature=-crt-static' cargo +nightly build --release -Z build-std=std,panic_abort -Z build-std-features=panic_immediate_abort --target x86_64-unknown-linux-musl

clean:
    cargo clean

podman_build:
    podman build -t ghcr.io/thasos/eloran:latest .
podman_push:
    podman push --sign-by thasos@thasmanie.fr ghcr.io/thasos/eloran:latest

nixshell shell='zsh':
    nix develop --command {{shell}}
