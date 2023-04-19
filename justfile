# par défaut : lance la 1ère recipe, sinon :
default:
    just --list --unsorted

run:
    cargo run -- -v

test:
    cargo test

build: test
    cargo +nightly build --release -Z build-std=std,panic_abort -Z build-std-features=panic_immediate_abort --target x86_64-unknown-linux-gnu

export PKG_CONFIG_SYSROOT_DIR := "/home/${USER}/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-musl"
build_musl: test
    RUSTFLAGS='-C target-feature=-crt-static'
    cargo +nightly build --release -Z build-std=std,panic_abort -Z build-std-features=panic_immediate_abort --target x86_64-unknown-linux-musl

clean:
    cargo clean
