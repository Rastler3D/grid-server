FROM rust:latest AS  builder

WORKDIR /usr/src

RUN USER=root cargo new task_manager

WORKDIR /usr/src/task_manager

COPY ./Cargo.toml ./Cargo.lock ./
COPY ../shared ../shared

RUN cargo build --release \
    && rm src/*.rs

COPY . ./

RUN rm ./target/release/deps/task_manager* \
    && cargo build --release

FROM debian:buster-slim
RUN apt-get update \
    && apt-get -y install libpq-dev \
    && apt-get -y install curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/task_manager/target/release/task_manager  /bin/

EXPOSE 8080

CMD ["/bin/task_manager"]

