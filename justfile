# par défaut : lance la 1ère recipe, sinon :
_default:
    just --list --unsorted

run:
    cargo run -- -v

test:
    # cargo insta test --review
    cargo test

review:
    cargo insta review

build:
    cargo +nightly build --release -Z build-std=std,panic_abort -Z build-std-features=panic_immediate_abort --target x86_64-unknown-linux-gnu

export PKG_CONFIG_SYSROOT_DIR := "/home/${USER}/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-musl"
build_musl:
    RUSTFLAGS='-C target-feature=-crt-static'
    cargo +nightly build --release -Z build-std=std,panic_abort -Z build-std-features=panic_immediate_abort --target x86_64-unknown-linux-musl

clean:
    cargo clean

podman_build:
    podman build -t ghcr.io/thasos/eloran:latest .
docker_build:
    @just podman_build

nixshell:
    nix-shell shell.nix --run 'zsh --emulate zsh'
