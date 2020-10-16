#! /bin/bash

bc=$(echo $1 | sed -e s/\.c$/\.bc/)
s=$(echo $1 | sed -e s/\.c$/\.s/)

if [[ $2 == "--release" ]]; then
  if [[ ! -e './target/release/rucc' ]]; then
    cargo build --release
  fi
  ./target/release/rucc $1
else
  cargo run $1
fi

if [[ $? == 0 ]]; then
  # opt-4.0 -std-link-opts $bc -o $bc 
  llc-10 $bc
  clang $s -lm
  rm -f $bc $s
fi
