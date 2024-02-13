# hadolint global ignore=DL3059
# builder
# need almost 4.5 GB and 6 minutes on a i5-8400
FROM docker.io/rustlang/rust:nightly-alpine AS builder
WORKDIR /opt/
# init a new cargo repo
RUN cargo new eloran
COPY src/ /opt/eloran/src
COPY Cargo.* /opt/eloran
# TODO use justfile ?
COPY justfile /opt/eloran
WORKDIR /opt/eloran
# no need for one layer here, it's just a builder
# dependencies
RUN apk --no-cache add just upx musl-dev pkgconfig glib-dev cairo-dev poppler-dev libarchive-dev
# nightly target for some build features
RUN rustup component add rust-src --toolchain nightly-x86_64-unknown-linux-musl
RUN RUSTFLAGS='-C target-feature=-crt-static' cargo +nightly build --release -Z build-std=std,panic_abort -Z build-std-features=panic_immediate_abort --target x86_64-unknown-linux-musl
# compress binary (only for network and disk size, it will be uncompressed in ram)
RUN upx target/x86_64-unknown-linux-musl/release/eloran

# runner
FROM docker.io/alpine:3.19
WORKDIR /opt/eloran
COPY --from=builder /opt/eloran/target/x86_64-unknown-linux-musl/release/eloran /opt/eloran
# TODO put thoses default files directly in the binary
# COPY ./src/css ./src/css
COPY ./src/images ./images

# poppler for pdf cover generation, libarchive for uncompression
RUN apk --no-cache add poppler-glib libarchive

# root in container is evil
RUN addgroup -g 10666 eloran \
 && adduser -D -u 10666 -G eloran eloran
RUN mkdir /opt/eloran/sqlite \
 && chown eloran /opt/eloran/sqlite
USER eloran

# start
# TODO catch ctrl+c in binary for gracefull and quick shutdown
CMD ["/opt/eloran/eloran"]
