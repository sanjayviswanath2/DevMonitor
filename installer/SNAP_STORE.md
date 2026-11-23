# Publish DevMonitor to the Snap Store

Snap name reserved in packaging: **`devmonitor`** (available on the store as of packaging time).

## 1. Create a Snapcraft account

1. Open [https://snapcraft.io/account](https://snapcraft.io/account)
2. Sign in with **Ubuntu One** (create an account if needed)
3. Agree to the developer terms

## 2. Install build tools (once)

```bash
sudo snap install snapcraft --classic
sudo snap install lxd
sudo lxd init --auto
# alternative build VM: sudo snap install multipass
```

## 3. Register, build, and publish

From the DevMonitor repo:

```bash
# Interactive Ubuntu One login in the terminal
bash scripts/publish-snap.sh --login

# Claim the store name (one-time)
bash scripts/publish-snap.sh --register

# Build the .snap from the release binary
bash scripts/publish-snap.sh

# Upload to the edge channel (safe first publish)
bash scripts/publish-snap.sh --upload

# When classic confinement is approved, promote to stable:
bash scripts/publish-snap.sh --release
```

Or in one go after tools are installed:

```bash
bash scripts/publish-snap.sh --all
```

## 4. Classic confinement (required)

DevMonitor is a system monitor, so the snap uses **`confinement: classic`**.

Classic desktop snaps must:
- ship the GTK/WebKit runtime inside the snap (`stage-packages`)
- enable Snapcraft ELF patching with `build-attributes: [enable-patchelf]`
- keep the GitHub source public so store reviewers can inspect `snapcraft.yaml`

Forum request thread:
[https://forum.snapcraft.io/t/classic-confinement-for-devmonitor/54290](https://forum.snapcraft.io/t/classic-confinement-for-devmonitor/54290)

Dashboard:
[https://dashboard.snapcraft.io/snaps/devmonitor/](https://dashboard.snapcraft.io/snaps/devmonitor/)

Until classic is approved, keep releases on **`edge`** (or `beta`). Stable releases of classic snaps are blocked without approval.

## 5. Install from the store

```bash
sudo snap install devmonitor --classic --channel=edge
# later:
sudo snap install devmonitor --classic
```

Store listing: [https://snapcraft.io/devmonitor](https://snapcraft.io/devmonitor)

## Artifacts

- Spec: `snap/snapcraft.yaml`
- GUI icon/desktop: `snap/gui/`
- Built snap: `installer/devmonitor_0.1.0_amd64.snap` (after build)
