#!/usr/bin/env bash
# spellchecker:ignore libc kversion

# We need the debug symbols of the libc6-dbg package available in the qemu image
# to be able to run Valgrind's memcheck. We can use the `/linux-image.sh` script
# from cross which is still present within the cross image.
set -xe

cd /

# shellcheck disable=SC2016
if ! grep 'libc6-dbg' /linux-image.sh; then
  rm -f /qemu/initrd.gz /qemu/kernel
fi

case $VALGRIND_REQUESTS_CROSS_TARGET in
riscv64gc-unknown-linux-gnu) arch="riscv64" ;;
*) arch="${VALGRIND_REQUESTS_CROSS_TARGET%%-*}" ;;
esac

/linux-image.sh "$arch"
