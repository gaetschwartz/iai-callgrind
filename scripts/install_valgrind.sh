#!/bin/bash -eux

# spell-checker: ignore waitretry connrefused

# This script is intended to be run on an ubuntu runner in the github ci

set -o pipefail

valgrind_version="${1:-3.26.0}"
ubuntu_version="ubuntu-$(lsb_release -r | awk '{print $2}')"
archive_name="valgrind-${valgrind_version}-x86_64-${ubuntu_version}.tar.gz"

# Don't install a newer libc6 version than the one that is already installed.
# Updating it without a restart might not be the safest thing to do. We just
# need the fitting libc6-dbg package which is required for example for the
# memcheck tool.
sudo apt-mark hold libc6
libc_version="$(dpkg-query -W -f='${Version}' libc6)"
sudo apt-get update

# Use a snapshot if the github runner libc version falls behind the latest
# ubuntu libc
if apt list --upgradable | grep libc6; then
  sudo apt-get install --update --assume-yes --no-install-recommends --no-upgrade --snapshot 20260302T000000Z libc6-dbg="${libc_version}"
else
  sudo apt-get install --update --assume-yes --no-install-recommends --no-upgrade libc6-dbg="${libc_version}"
fi

gh release download --repo gungraun/valgrind-builder -p "${archive_name}*"
sha256sum -c "${archive_name}.sha256"

sudo tar xzf "$archive_name" -C /
