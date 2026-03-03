FROM rust

ENV ROCKET_ADDRESS=0.0.0.0
ENV ROCKET_PORT=8080

EXPOSE 8080

ADD . /app
WORKDIR /app

RUN cargo install cargo-watch
RUN cargo build
CMD ["cargo", "watch", "-x", "run"]