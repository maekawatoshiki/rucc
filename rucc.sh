#! /bin/bash
bc=$(echo $1 | sed -e s/\.c$/\.bc/)
s=$(echo $1 | sed -e s/\.c$/\.s/)
cargo run $1
# ./target/release/rucc $1
if [[ $? == 0 ]]; then
  # opt-3.8 -std-link-opts $ll -o $ll
  llc-3.8 $bc
  clang $s -lm
  rm -f $bc $s
fi
