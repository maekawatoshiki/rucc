FROM ubuntu:16.04

RUN \
  # sed -i".bak" -e 's/\/\/archive.ubuntu.com/\/\/ftp.jaist.ac.jp/g' /etc/apt/sources.list && \
  apt-get update && \
  apt-get upgrade -y && \
  apt-get install zlib1g-dev -y && \
  apt-get install clang-3.8 clang-3.8-doc libclang-common-3.8-dev libclang-3.8-dev libclang1-3.8 libclang1-3.8-dbg libllvm3.8 libllvm3.8-dbg lldb-3.8 llvm-3.8 llvm-3.8-dev llvm-3.8-doc llvm-3.8-examples llvm-3.8-runtime clang-format-3.8 python-clang-3.8 lldb-3.8-dev opt libedit-dev build-essential make -y; \
  curl -s https://static.rust-lang.org/rustup.sh | sh -s -- --disable-sudo -y

ADD . /opt/rucc

WORKDIR /opt/rucc

RUN ./rucc.sh
