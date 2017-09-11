FROM jimmycuadra/rust:latest

RUN \
  sed -i".bak" -e 's/\/\/archive.ubuntu.com/\/\/ftp.jaist.ac.jp/g' /etc/apt/sources.list && \
  apt-get update && \
  apt-get upgrade -y && \
  apt-get install zlib1g-dev -y && \
  apt-get install clang-3.9 clang-3.9-doc libclang-common-3.9-dev libclang-3.9-dev libclang1-3.9 libclang1-3.9-dbg libllvm3.9 libllvm3.9-dbg llvm-3.9 llvm-3.9-dev llvm-3.9-doc llvm-3.9-examples llvm-3.9-runtime clang-format-3.9 python-clang-3.9 opt libedit-dev build-essential make -y; \
  ln -s /usr/bin/clang-3.9 /usr/bin/clang; \
  ln -s /usr/bin/clang++-3.9 /usr/bin/clang++; \
  ln -s /usr/bin/llvm-config-3.9 /usr/bin/llvm-config;


ADD . /opt/rucc

WORKDIR /opt/rucc

# RUN cargo test
