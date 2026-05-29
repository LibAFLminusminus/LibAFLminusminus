import "libafl-qemu.just"

# Useful rules to build libpng for multiple architecture.

OPTIMIZATIONS := env("OPTIMIZATIONS", if ARCH == "ppc" { "no" } else { "yes" })
REAL_CC := if ARCH == "x86_64" { "gcc" } else { "arm-linux-gnueabi-gcc" }

DEPS_DIR := TARGET_DIR / "deps"

[unix]
target_dir:
    mkdir -p {{ TARGET_DIR }}

[unix]
deps_dir:
    mkdir -p {{ DEPS_DIR }}

[unix]
arch_dir:
    mkdir -p {{ ARCH }}

[unix]
zlib_wget: deps_dir
    #!/bin/bash

    if [ -e "{{ DEPS_DIR }}/zlib-1.2.13" ]; then
        echo "zlib sources already present, skipping download"
        exit 0
    fi

    wget \
        -O "{{ DEPS_DIR }}/zlib-1.2.13.tar.gz" \
        https://github.com/madler/zlib/releases/download/v1.2.13/zlib-1.2.13.tar.gz

    tar \
        zxvf {{ DEPS_DIR }}/zlib-1.2.13.tar.gz \
        -C {{ DEPS_DIR }}

[unix]
zlib: zlib_wget
    #!/bin/bash

    source {{ DOTENV }}

    if [ -e "{{ TARGET_DIR }}/build-zlib/zlib/lib/libz.a" ]; then
        echo "zlib already built, skipping"
        exit 0
    fi

    mkdir -p {{ TARGET_DIR }}/build-zlib/

    cd {{ TARGET_DIR }}/build-zlib/ && \
        CC={{REAL_CC}} \
        CFLAGS="$CROSS_CFLAGS -Wno-error" \
        {{ DEPS_DIR }}/zlib-1.2.13/configure \
            --prefix=./zlib

    make -j install

[unix]
libpng_wget: deps_dir
    #!/bin/bash

    if [ -e "{{ DEPS_DIR }}/libpng-1.6.37" ]; then
        echo "libpng sources already present, skipping download"
        exit 0
    fi

    wget \
        -O "{{ DEPS_DIR }}/v1.6.37.tar.gz" \
        https://github.com/glennrp/libpng/archive/refs/tags/v1.6.37.tar.gz

    tar \
        -xvf "{{ DEPS_DIR }}/v1.6.37.tar.gz" \
        -C {{ DEPS_DIR }}

[unix]
libpng: arch_dir zlib libpng_wget
    #!/bin/bash

    source {{ DOTENV }}

    if [ -e "{{ TARGET_DIR }}/build-png/.libs/libpng16.a" ]; then
        echo "libpng already built, skipping"
        exit 0
    fi

    mkdir -p {{TARGET_DIR}}/build-png/

    cd {{ TARGET_DIR }}/build-png/ && \
        CC=$CROSS_CC \
        CFLAGS="$CROSS_CFLAGS -I"{{ TARGET_DIR }}/build-zlib/zlib/lib"" \
        LDFLAGS=-L"{{ TARGET_DIR }}/build-zlib/zlib/lib" \
        {{ DEPS_DIR }}/libpng-1.6.37/configure \
            --enable-shared=no \
            --with-pic=yes \
            --enable-hardware-optimizations={{ OPTIMIZATIONS }} \
            --host={{ ARCH }}

    make -j -C {{ TARGET_DIR }}/build-png/
