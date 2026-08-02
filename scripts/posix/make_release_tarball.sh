#!/bin/sh

set -o errexit
set -o nounset

cd `dirname $0`
cd ..
cd ..

cargo build --release
echo

KERNEL=`uname -s`
ARCH=`uname -m`
OS=unknown
if echo "${KERNEL}" | grep -qi "linux"; then
  OS=linux
elif echo "${KERNEL}" | grep -qi "darwin"; then
  OS=macos
fi

PKG_VER=`cargo pkgid | cut -d# -f2`

TARBALL_NAME="pseudochef-${OS}-${ARCH}-${PKG_VER}.tar.gz"

echo "Detected kernel: ${KERNEL}"
echo "Detected architecture: ${ARCH}"
echo "Inferred OS: ${OS}"
echo
echo "Package version is: ${PKG_VER}"
echo
echo "Writing to: ${TARBALL_NAME}"
echo

cp target/release/pseudochef static
cd static
tar -czvf "../${TARBALL_NAME}" *
