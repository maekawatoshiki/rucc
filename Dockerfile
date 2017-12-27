FROM jimmycuadra/rust:latest

RUN \
  sed -i".bak" -e 's/\/\/archive.ubuntu.com/\/\/ftp.jaist.ac.jp/g' /etc/apt/sources.list && \
  apt-get update && \
  apt-get upgrade -y && \
  apt-get install zlib1g-dev -y && \
  apt-get install clang-4.0 llvm-4.0 llvm-4.0-dev opt libedit-dev build-essential make -y; \
  ln -s /usr/bin/clang-4.0 /usr/bin/clang; \
  ln -s /usr/bin/clang++-4.0 /usr/bin/clang++; \
  ln -s /usr/bin/llvm-config-4.0 /usr/bin/llvm-config;


ADD . /opt/rucc

WORKDIR /opt/rucc

# RUN cargo test
