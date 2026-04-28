#!/bin/bash
# spell-checker: ignore DESTDIR autogen

set -ex

export CC="${CROSS_TOOLCHAIN_PREFIX}gcc"
export LD="${CROSS_TOOLCHAIN_PREFIX}ld"
export AR="${CROSS_TOOLCHAIN_PREFIX}ar"

which "$CC" "$LD" "$AR"

[[ -n "$VALGRIND_REQUESTS_CROSS_TARGET" ]] || {
  echo "VALGRIND_REQUESTS_CROSS_TARGET environment variable is not defined"
  exit 1
}

case $VALGRIND_REQUESTS_CROSS_TARGET in
riscv64gc-unknown-linux-gnu) HOST="riscv64-linux-gnu" ;;
*-*-*-*) HOST="$VALGRIND_REQUESTS_CROSS_TARGET" ;;
*-*-*) HOST="$VALGRIND_REQUESTS_CROSS_TARGET" ;;
*)
  echo "Invalid target specification for VALGRIND_REQUESTS_CROSS_TARGET: '$VALGRIND_REQUESTS_CROSS_TARGET'" >&2
  exit 1
  ;;
esac

cd ~/valgrind/valgrind-"${VALGRIND_REQUESTS_CROSS_VALGRIND_VERSION}"

dest_dir="/valgrind"
target_dir="/target/valgrind/${VALGRIND_REQUESTS_CROSS_TARGET}"

mkdir "$dest_dir"

./autogen.sh

# According to valgrind/configure file, the VALGRIND_REQUESTS_CROSS_TARGET is
# supported as is for the --host variable. If the target is not supported by
# valgrind, configure will exit with an error.
./configure --prefix="$target_dir" \
  --host="${HOST}"

make -j4
make -j4 install DESTDIR="$dest_dir"
