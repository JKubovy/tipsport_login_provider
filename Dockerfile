# Build Stage
FROM ekidd/rust-musl-builder AS builder
RUN sudo apt-get update -y
RUN sudo apt-get install -y upx

# Build just rust dependencies
WORKDIR /usr/src/tipsport_request_provider
RUN USER=root cargo init
COPY Cargo.toml Cargo.lock ./
# RUN echo "fn main() {}" > ./dummy.rs
# RUN sed -i 's#src/main.rs#dummy.rs#' Cargo.toml
# RUN cargo build --release --target x86_64-unknown-linux-musl
# RUN rm ./dummy.rs
# RUN sed -i 's#dummy.rs#src/main.rs#' Cargo.toml

# Build actual program
COPY src ./src
RUN cargo build --release --target x86_64-unknown-linux-musl
RUN upx --best --lzma /usr/src/tipsport_request_provider/target/x86_64-unknown-linux-musl/release/tipsport_request_provider

# Copy binary to final docker
#FROM scratch
FROM alpine:latest
COPY --from=builder --chown=root:root /usr/src/tipsport_request_provider/target/x86_64-unknown-linux-musl/release/tipsport_request_provider /tipsport_request_provider
COPY Rocket.toml /
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
USER 1000
EXPOSE 80
ENTRYPOINT [ "/tipsport_request_provider" ]
