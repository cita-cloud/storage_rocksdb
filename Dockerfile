FROM rust:slim-buster AS buildstage
WORKDIR /build
RUN /bin/sh -c set -eux;\
    rustup component add rustfmt;\
    apt-get update;\
    apt-get install -y --no-install-recommends librocksdb-dev libsnappy-dev liblz4-dev libzstd-dev clang git;\
    rm -rf /var/lib/apt/lists/*;
COPY . /build/
RUN ROCKSDB_LIB_DIR=/usr/lib/ ROCKSDB_STATIC=1 SNAPPY_LIB_DIR=/usr/lib/x86_64-linux-gnu/ SNAPPY_STATIC=1 LZ4_LIB_DIR=/usr/lib/x86_64-linux-gnu/ LZ4_STATIC=1 ZSTD_LIB_DIR=/usr/lib/x86_64-linux-gnu/ ZSTD_STATIC=1 cargo build --release
FROM debian:buster-slim
COPY --from=buildstage /build/target/release/storage /usr/bin/
CMD ["storage"]
