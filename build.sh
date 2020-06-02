#!/usr/bin/env sh

if [ -z ${ARM6_GCC} ]; then
    echo "ARM6_GCC is unset";
    exit 1;
fi

DEBUG=""

if ( getopts ":r" opt); then
    DEBUG="--release";
    echo "Building in release mode"
fi

RUSTFLAGS="-C linker=$ARM6_GCC" cargo build --target arm-unknown-linux-gnueabihf $DEBUG
