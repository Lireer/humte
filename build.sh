#!/usr/bin/env sh

if [ -z ${ARM6_GCC} ]; then
    echo "ARM6_GCC is unset";
    exit 1;
fi

COMMAND="build"
DEBUG=""

while getopts ":r" opt; do
    case $opt in
        r)
            DEBUG="--release";
            echo "Building in release mode";
            ;;
    esac
done

RUSTFLAGS="-C linker=$ARM6_GCC" cargo $COMMAND --target arm-unknown-linux-gnueabihf $DEBUG
