FROM rust:latest AS  builder

WORKDIR /usr/src

RUN USER=root cargo new node

WORKDIR /usr/src/node

COPY ./Cargo.toml ./Cargo.lock ./
COPY ./../shared ../

RUN cargo build --release \
    && rm src/*.rs

COPY . ./

RUN rm ./target/release/deps/node* \
    && cargo build --workspace --release

FROM debian:buster-slim
RUN apt-get update \
    && apt-get -y install libpq-dev \
    && apt-get -y install curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/node/target/release/node /usr/src/node/target/release/executor /bin/

EXPOSE 8080

CMD ["/bin/node"]

