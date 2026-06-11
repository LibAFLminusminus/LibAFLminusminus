#!/bin/bash

set -eu
mkdir corpus || true
if [ ! -d "sqlite3" ]; then
    curl 'https://sqlite.org/src/tarball/sqlite.tar.gz?r=c78cbf2e86850cc6' -o sqlite3.tar.gz && mkdir sqlite3 && pushd sqlite3 && tar xzf ../sqlite3.tar.gz --strip-components 1 && popd
    find ./sqlite3 -name "*.test" -exec cp {} corpus/ \;
fi

if [ "$1" = "release" ]; then
  cargo build --release
  DIR=release
elif [ "$1" = "dev" ]; then
  cargo build
  DIR=debug
else
    echo "Incorrect profile: $1. Either use 'dev' or 'release'."
    exit 1
fi

export CC="$PWD/target/$DIR/libafl_cc"
export CXX="$PWD/target/$DIR/libafl_cxx"
export CFLAGS='--libafl'
export CXXFLAGS='--libafl'
export CFLAGS="$CFLAGS -DSQLITE_MAX_LENGTH=128000000 \
               -DSQLITE_MAX_SQL_LENGTH=128000000 \
               -DSQLITE_MAX_MEMORY=25000000 \
               -DSQLITE_PRINTF_PRECISION_LIMIT=1048576 \
               -DSQLITE_DEBUG=1 \
               -DSQLITE_MAX_PAGE_COUNT=16384 \
               -Wno-error=implicit-function-declaration"
pushd sqlite3

if [ ! -f "Makefile" ]; then
    echo "Run configure..."
    ./configure
fi
make sqlite3.c
make -j"$(nproc)"
popd

echo "Compiling 'ossfuzz' with profile '$1'..."

eval "./target/$DIR/libafl_cc --libafl -I ./sqlite3 -c ./sqlite3/test/ossfuzz.c -o ./sqlite3/test/ossfuzz.o"
eval "./target/$DIR/libafl_cxx --libafl -o ossfuzz ./sqlite3/test/ossfuzz.o ./sqlite3/sqlite3.o -pthread -ldl -lz"

echo "'ossfuzz' is ready."
