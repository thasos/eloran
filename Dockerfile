# hadolint global ignore=DL3059
# builder
# need almost 4.5 GB and 5 minutes on a i7-8700K
FROM docker.io/rust:1.88-alpine3.22 AS builder

# dependencies
RUN apk --no-cache add just upx musl-dev \
                       pkgconfig glib-dev cairo-dev poppler-dev libarchive-dev \
 && rustup default nightly \
 && rustup component add rust-src --toolchain nightly-x86_64-unknown-linux-musl \
 && cargo install grass

WORKDIR /opt/

# init a new cargo repo
RUN cargo new eloran
COPY src/ /opt/eloran/src
COPY css/ /opt/eloran/css
COPY sass/ /opt/eloran/sass
COPY Cargo.* /opt/eloran
COPY site.webmanifest /opt/eloran
COPY justfile /opt/eloran

WORKDIR /opt/eloran

RUN just build_musl

# compress binary (only for network and disk size, it will be uncompressed in ram)
RUN upx target/x86_64-unknown-linux-musl/release/eloran

# runner
FROM docker.io/alpine:3.22

LABEL "org.opencontainers.image.base.name" = "ghcr.io/thasos/eloran" \
      "org.opencontainers.image.created" = "2024-12-20T17:45:05Z" \
      "org.opencontainers.image.revision" = "813e06b" \
      "org.opencontainers.image.source" = "https://github.com/thasos/eloran" \
      "org.opencontainers.image.url" = "https://github.com/thasos/eloran/pkgs/container/eloran" \
      "org.opencontainers.image.version" = "0.3.1"

WORKDIR /opt/eloran
COPY --from=builder /opt/eloran/target/x86_64-unknown-linux-musl/release/eloran /opt/eloran

COPY ./images ./images
COPY ./fonts ./fonts

# poppler for pdf cover generation, libarchive for uncompression
RUN apk --no-cache add poppler-glib libarchive

# TODO handle user id at start (see issue #12)
# root in container is evil
# RUN addgroup -g 10666 eloran \
#  && adduser -D -u 10666 -G eloran eloran
# RUN mkdir /opt/eloran/sqlite \
#  && chown eloran /opt/eloran/sqlite \
#  && chmod +r /opt/eloran/images/* /opt/eloran/fonts/*
# USER eloran

RUN mkdir /opt/eloran/sqlite

# start
# TODO catch ctrl+c in binary for gracefull and quick shutdown
CMD ["/opt/eloran/eloran"]
