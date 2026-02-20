#!/bin/bash -eux

# spell-checker: ignore waitretry

# This script is intended to be run on an ubuntu runner in the github ci

set -o pipefail

valgrind_version="${1:-3.26.0}"
wget=('wget' '--waitretry=10' '--retry-on-host-error')

ubuntu_version="ubuntu-$(lsb_release -r | awk '{print $2}')"

latest=null
retries=10
while [[ "$latest" == "null" ]]; do
  if ((retries == 0)); then
    echo "Maximum retries reached"
    exit 1
  else
    latest=$(gh release view --repo gungraun/valgrind-builder --json tagName | jq -r '.tagName')
    retries=$((retries - 1))
  fi
done

archive_name="valgrind-${valgrind_version}-x86_64-${ubuntu_version}.tar.gz"
archive_url="https://github.com/gungraun/valgrind-builder/releases/download/${latest}/${archive_name}"
sha_url="${archive_url}.sha256"

# Don't install a newer libc6 version than the one that is already installed.
# Updating it without a restart might not be the safest thing to do. We just
# need the fitting libc6-dbg package which is required for example for the
# memcheck tool.
sudo apt-mark hold libc6
libc_version="$(dpkg-query -W -f='${Version}' libc6)"
sudo apt-get update
sudo apt-get install --update --assume-yes --no-install-recommends --no-upgrade --snapshot 20260128T000000Z libc6-dbg="${libc_version}"

"${wget[@]}" "$archive_url"
"${wget[@]}" "$sha_url"

sha256sum -c "${archive_name}.sha256"

sudo tar xzf "$archive_name" -C /
