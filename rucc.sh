#! /bin/bash

ll=$(echo $1 | sed -e s/\.c$/\.ll/)
s=$(echo $1 | sed -e s/\.c$/\.s/)
cargo run $1
llc-3.8 $ll
clang $s
