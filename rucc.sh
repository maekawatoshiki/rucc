#! /bin/bash
ll=$(echo $1 | sed -e s/\.c$/\.ll/)
s=$(echo $1 | sed -e s/\.c$/\.s/)
cargo run $1
# ./target/release/rucc $1
if [[ $? == 0 ]]; then
  # opt-3.8 -std-link-opts $ll -o $ll
  llc-3.8 $ll
  clang $s -lm
  rm -f $ll $s
fi
