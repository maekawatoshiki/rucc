FROM ubuntu:16.04

RUN \
  # sed -i".bak" -e 's/\/\/archive.ubuntu.com/\/\/ftp.jaist.ac.jp/g' /etc/apt/sources.list && \
  apt-get update && \
  apt-get upgrade -y && \
  apt-get install curl zlib1g-dev -y && \
  apt-get install llvm-3.8 opt libedit-dev build-essential make -y; \
  curl -s https://static.rust-lang.org/rustup.sh | sh -s -- --disable-sudo -y

ADD . /opt/rucc

WORKDIR /opt/rucc

RUN ./rucc.sh
