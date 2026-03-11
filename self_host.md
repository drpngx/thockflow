# Self-Hosting ThockFlow with Kata Containers on WSL2

Run ThockFlow in a hardware-isolated Kata VM behind a Cloudflare Tunnel, all inside WSL2.

**Architecture:** Bazel builds an OCI image → Podman loads it → Kata runs it in a microVM → `cloudflared` exposes it.

---

## Prerequisites

- Windows 11 (Build 22000+) with WSL2
- Ubuntu 22.04+ in WSL2
- Bazel installed (for building the OCI image)

---

## 1. WSL2: Enable Nested Virtualization and systemd

Kata needs KVM, which requires nested virtualization. Podman/Kata need cgroups v2 via systemd.

### Windows side: `C:\Users\<YourUser>\.wslconfig`

```ini
[wsl2]
nestedVirtualization=true
kernelCommandLine=cgroup_no_v1=all systemd.unified_cgroup_hierarchy=1
```

### Ubuntu side: `/etc/wsl.conf`

systemd is required for cgroup management. The `kernelCommandLine` forces cgroup v2
(WSL2 defaults to cgroup v1, which Kata v3.x does not support).

```ini
[boot]
systemd=true
```

Restart WSL from PowerShell:

```powershell
wsl --shutdown
```

Reopen your Ubuntu terminal, then verify:

```bash
# No lsmod
sudo apt install cpu-checker
sudo kvm-ok

# Should show systemd as PID 1
ps -p 1 -o comm=

# check cgroup
mount|grep cgroup2

# Should print "cgroup2fs" (NOT "tmpfs")
stat -fc %T /sys/fs/cgroup

# Should print "cgroupVersion: v2"
podman info | grep cgroupVersion
```

---

## 2. Install Podman

```bash
sudo apt update
sudo apt install -y podman uidmap slirp4netns
```

Verify:

```bash
podman --version
podman info | grep -i cgroup  # should show cgroupVersion: v2
```

---

## 3. Install Kata Containers

This installs to `/opt/kata/`. The runtime binary is at `/opt/kata/bin/kata-runtime`.

```
server/kata_install.sh
```

### Verify Kata

```bash
/opt/kata/bin/kata-runtime check
<fails with vsock, which is disabled in the WSL2 kernels>
```

### Fix Kata vsock

Create or edit `/etc/containers/containers.conf`:

```bash
[engine.runtimes]
kata = ["/usr/local/bin/kata-runtime"]
EOF
```
(maybe useless)

# 1. Create the high-priority config directory
sudo mkdir -p /etc/kata-containers

# 2. Force the /etc/ config to point to Cloud Hypervisor
sudo ln -sf /opt/kata/share/defaults/kata-containers/configuration-clh.toml /etc/kata-containers/configuration.toml

# 3. Overwrite the fallback /opt/ symlink just to be absolutely certain
sudo ln -sf /opt/kata/share/defaults/kata-containers/configuration-clh.toml /opt/kata/share/defaults/kata-containers/configuration.toml

### Podman is borked
podman wants to call kata-runtime delete, create, etc. But they are not there, since kata implement shim-v2

### Use docker or k3s
nerdctl, etc: all work with containerd.
k3s will use one pod (VM) with two containers, docker-compose will use two VMs.
```
sudo ln -sf /opt/kata/bin/containerd-shim-kata-v2 /usr/local/bin/containerd-shim-kata-v2
```

Edit `/etc/docker/daemon.json`:
```
{
  "runtimes": {
    "kata": {
      "runtimeType": "io.containerd.kata.v2"
    }
  }
}
```

```
uname -r
docker run --rm --runtime=kata alpine uname -r
```
Should have different versions.

---

## 4. Create a Cloudflare Tunnel

1. Go to the [Cloudflare Zero Trust dashboard](https://one.dash.cloudflare.com/)
2. Navigate to **Networks → Routes → Create a tunnel**
3. Choose **Cloudflared** connector
4. Copy the tunnel token (it's in the cloudflared token)
5. Configure the public hostname to point to `http://thockflow:8080`

---

## 5. Build and Run

Everything is wrapped in a single Bazel target. It builds both OCI images
(thockflow + cloudflared), loads them into Podman, and starts `podman-compose`.

```bash
# Build and run (add -d to detach)
envgpg -e CF_TUNNEL_THOCKFLOW bazel run //server:serve
```

This builds:
- The optimized Rust server binary (cross-compiled for linux/amd64)
- Optimized + Brotli-compressed WASM assets
- A distroless container image (no shell, runs as UID 65532)
- Pulls the `cloudflare/cloudflared` OCI image

The compose file puts thockflow on an internal-only network (`thockflow_internal`)
with no external access. Only cloudflared bridges both the internal and external
networks, acting as the sole ingress point.

### Verify

```bash
podman ps
podman logs thockflow
podman logs cloudflared-thockflow
```

### Stop

```bash
podman-compose -f podman-compose.yaml down
```

---

## Quick Reference

| Command | Description |
|---------|-------------|
| `CF_TUNNEL_TOKEN=... bazel run //server:serve` | Build + load + start everything |
| `CF_TUNNEL_TOKEN=... bazel run //server:serve -- -d` | Same, but detached |
| `podman-compose -f podman-compose.yaml down` | Stop all services |
| `podman logs thockflow` | View server logs |
| `/opt/kata/bin/kata-runtime check` | Verify Kata works |

## Troubleshooting

### "KVM not available"
- Ensure `nestedVirtualization=true` is in `.wslconfig` and you've restarted WSL
- Run `lsmod | grep kvm` — if empty, run `sudo modprobe kvm_intel` (or `kvm_amd`)
- On AMD CPUs, you may need to enable SVM in BIOS

### "cgroup: failed to find" errors
- Ensure `systemd=true` is in `/etc/wsl.conf` and you've restarted WSL
- Verify with `ps -p 1 -o comm=` — should show `systemd`

### Podman can't pull cloudflared image
- Ensure `/etc/containers/registries.conf` includes `docker.io` in `unqualified-search-registries`

### Container starts but site unreachable
- Check `podman logs thockflow` for server errors
- Verify the IP in `config.yml` matches what `podman inspect thockflow` shows
- The nonroot image listens on `127.0.0.1:8080` — this works when cloudflared runs in the same pod/network, but for the manual setup you need the container's network IP. Override with:

```bash
podman run -d \
  --name thockflow \
  --runtime kata \
  --network thockflow-net \
  --ip 10.88.0.10 \
  --cap-drop ALL \
  -e HTTP_LISTEN_ADDR=0.0.0.0:8080 \
  localhost/thockflow:latest
```
