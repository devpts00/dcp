FROM rust:latest

RUN rustup default stable
RUN apt update && apt install -y \
    libjemalloc2 \
    graphviz golang-go google-perftools libgoogle-perftools-dev \
    libmimalloc-dev \
    bc heaptrack time
