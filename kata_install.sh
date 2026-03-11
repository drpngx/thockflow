# 1. Define Version
export KATA_VERSION=$(curl -fsSL https://api.github.com/repos/kata-containers/kata-containers/releases \
  | jq -r '.[].tag_name | select(contains("-") | not)' \
  | sort -V \
  | tail -1)

sudo apt-get install -y zstd

# 2. Download the static release
# 1. Install extraction tool if missing
sudo apt-get update && sudo apt-get install -y zstd

# 2. Define the Version
export KATA_VERSION="3.27.0"

# 3. Download the static binary (AMD64)
# Note the change from .tar.xz to .tar.zst
echo "Downloading Kata Containers ${KATA_VERSION}..."
curl -L -o "kata-static-${KATA_VERSION}-x86_64.tar.zst" \
  "https://github.com/kata-containers/kata-containers/releases/download/${KATA_VERSION}/kata-static-${KATA_VERSION}-amd64.tar.zst"

# 4. Extract to root
# We use --zstd flag for tar
echo "Extracting binaries to /opt/kata..."
sudo mkdir -p /opt/kata
sudo tar --zstd -xvf "kata-static-${KATA_VERSION}-x86_64.tar.zst" -C /

# 5. Link the runtime
sudo ln -sf /opt/kata/bin/kata-runtime /usr/local/bin/kata-runtime
