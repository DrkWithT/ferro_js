#!/bin/sh

warmsc="$1";
runc="$2";
filepath="$3";

if [[ $# -ne 3 ]]; then
    printf "Expected \x1b[1;31m2\x1b[0m arguments: warmup-count, runs-count, and relative-file-path.\n";
    exit 1;
fi

hyperfine --warmup=$warmsc --runs=$runc -N --sort "mean-time" -L rt qjs,"./target/release/ferrojs",boa "{rt} $filepath";
