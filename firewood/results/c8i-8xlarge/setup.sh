#!/bin/bash
set -euxo pipefail
sudo apt-get update
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y build-essential perl pkg-config curl linux-tools-common linux-tools-aws
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
curl -fsSL https://go.dev/dl/go1.26.8.linux-amd64.tar.gz -o /tmp/go.tgz
sudo tar -C /usr/local -xzf /tmp/go.tgz
rm /tmp/go.tgz
mkdir -p ~/dbs ~/results
/usr/local/go/bin/go version
~/.cargo/bin/rustc --version
lscpu -e=CPU,CORE,SOCKET
free -b
df -B1 /
