FROM rust:latest AS  builder

WORKDIR /usr/src

RUN USER=root cargo new scheduler

WORKDIR /usr/src/scheduler

COPY ./Cargo.toml ./Cargo.lock ./
COPY ../shared ../shared

RUN cargo build --release \
    && rm src/*.rs

COPY . ./

RUN rm ./target/release/deps/scheduler* \
    && cargo build --release

FROM debian:buster-slim
RUN apt-get update \
    && apt-get -y install libpq-dev \
    && apt-get -y install curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/scheduler/target/release/scheduler  /bin/

EXPOSE 8080

CMD ["/bin/scheduler"]

