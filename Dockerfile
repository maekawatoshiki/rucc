FROM ubuntu:16.04

# sed -i".bak" -e 's/\/\/archive.ubuntu.com/\/\/ftp.jaist.ac.jp/g' /etc/apt/sources.list && \
RUN ["/bin/bash", "-c", "\
  set -eux; \

  apt-get update && \
  apt-get upgrade -y && \
  apt-get install -y zlib1g-dev wget libssl-dev pkg-config cmake zlib1g-dev curl && \

  wget \"https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init\"; \
  chmod +x rustup-init; \
  ./rustup-init -y --no-modify-path --default-toolchain nightly; \
  RUSTUP_HOME=~/.cargo/bin/rustup; \
  CARGO_HOME=~/.cargo/bin/cargo; \
  chmod -R a+w $RUSTUP_HOME $CARGO_HOME; \
  rm rustup-init; \
  source ~/.cargo/env; \
  cargo install cargo-tarpaulin; \

  apt-get install clang-4.0 llvm-4.0 llvm-4.0-dev opt libedit-dev build-essential make -y; \
  ln -s /usr/bin/clang-4.0 /usr/bin/clang; \
  ln -s /usr/bin/clang++-4.0 /usr/bin/clang++; \
  ln -s /usr/bin/llvm-config-4.0 /usr/bin/llvm-config;"]



WORKDIR /opt/rucc
