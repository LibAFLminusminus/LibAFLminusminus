#!/bin/sh
#
# Replacement for https://github.com/nyx-fuzz/packer/blob/main/linux_initramfs/pack.sh.
# The upstream version hardcodes Debian/Ubuntu library paths which do not exist on other distributions.
# This version resolves the same libraries via `ldconfig -p`, so it
# works regardless of the host distribution's filesystem layout.
#
# Derived from the AGPL-3.0 packer project by Sergej Schumilo and
# Cornelius Aschermann, 2019.
# This file inherists the same licence.

if ! [ -x "$(command -v cpio)" ]; then
  echo 'Error: cpio is not installed.' >&2
  exit 1
fi

if ! [ -x "$(command -v gzip)" ]; then
  echo 'Error: gzip is not installed.' >&2
  exit 1
fi

if ! [ -x "$(command -v ldconfig)" ]; then
  echo 'Error: ldconfig is not on PATH (need glibc).' >&2
  exit 1
fi

set -e

# Resolve a shared library to an absolute path with ldconfig.
# $1 = soname (like libc.so.6)
# $2 = "32" or "64"
find_lib() {
    _soname="$1"
    _bits="$2"
    if [ "$_bits" = "64" ]; then
        _path=$(ldconfig -p | awk -v s="$_soname" '$1==s && /x86-64/ {print $NF; exit}')
    else
        _path=$(ldconfig -p | awk -v s="$_soname" '$1==s && !/x86-64/ {print $NF; exit}')
    fi
    if [ -z "$_path" ] || [ ! -e "$_path" ]; then
        echo "Error: cannot locate ${_bits}-bit $_soname via ldconfig. Install glibc and its 32-bit counterpart (lib32-glibc / libc6-i386 / glibc.i686)." >&2
        exit 1
    fi
    printf '%s\n' "$_path"
}

cd ../packer/linux_x86_64-userspace/
sh compile_loader.sh
cd -
cp ../packer/linux_x86_64-userspace/bin64/loader rootTemplate/loader
chmod +x rootTemplate/loader
mkdir -p rootTemplate/lib/
mkdir -p rootTemplate/lib64/
mkdir -p rootTemplate/lib/i386-linux-gnu/
mkdir -p rootTemplate/lib/x86_64-linux-gnu/
mkdir -p rootTemplate/lib32/

cp -L "$(find_lib ld-linux.so.2 32)" rootTemplate/lib/ld-linux.so.2
cp -L "$(find_lib ld-linux-x86-64.so.2 64)" rootTemplate/lib64/ld-linux-x86-64.so.2
cp -L "$(find_lib libdl.so.2 64)" rootTemplate/lib/x86_64-linux-gnu/libdl.so.2
cp -L "$(find_lib libc.so.6 64)" rootTemplate//lib/x86_64-linux-gnu/libc.so.6
cp -L "$(find_lib libc.so.6 32)"  rootTemplate//lib32/libc.so.6
cp -L "$(find_lib ld-linux.so.2 32)" rootTemplate/lib/ld-linux.so.2
cp -L "$(find_lib libdl.so.2 32)" rootTemplate/lib32/libdl.so.2

# fix nasty nss bugs (getpwnam_r, ...)
cp -L "$(find_lib libnss_compat.so.2 64)" rootTemplate//lib/x86_64-linux-gnu/

cp -r "rootTemplate" "init"
sed '/START/c\./loader' init/init_template > init/init
chmod 755 "init/init"
cd "init"

find . -print0 | cpio --null -ov --format=newc  2> /dev/null | gzip -9 > "../init.cpio.gz" 2> /dev/null
cd ../
rm -r ./init/


cp -r "rootTemplate" "init"
sed '/START/c\sh' init/init_template > init/init
chmod 755 "init/init"
cd "init"

find . -print0 | cpio --null -ov --format=newc  2> /dev/null | gzip -9 > "../init_debug_shell.cpio.gz"  2> /dev/null
cd ../
rm -r ./init/

rm -r rootTemplate/lib/
rm -r rootTemplate/lib64/
rm -r rootTemplate/lib32/
rm rootTemplate/loader
