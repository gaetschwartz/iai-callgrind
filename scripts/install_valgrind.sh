#!/bin/bash -ex

set -o pipefail

valgrind_version="${1:-3.26.0}"

ubuntu_version="ubuntu-$(lsb_release -r | awk '{print $2}')"

latest=null
retries=10
while [[ "$latest" == "null" ]]; do
  if ((retries == 0)); then
    echo "Maximum retries reached"
    exit 1
  else
    latest=$(curl --retry 3 -s https://api.github.com/repos/gungraun/valgrind-builder/releases/latest | jq -r .tag_name)
    retries=$((retries - 1))
  fi
done

archive_name="valgrind-${latest}-${ubuntu_version}-${valgrind_version}.tar.gz"
archive_url="https://github.com/gungraun/valgrind-builder/releases/download/${latest}/${archive_name}"
sha_url="${archive_url}.sha256"

sudo apt-get update
# libc6-dbg is required for the memcheck tool
sudo apt-get install --assume-yes --no-install-recommends libc6-dbg

wget "$archive_url"
wget "$sha_url"
sha256sum -c "${archive_name}.sha256"

sudo tar xzf "$archive_name" -C /
