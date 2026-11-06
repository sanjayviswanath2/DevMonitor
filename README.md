# DevMonitor

Linux developer environment doctor + system monitor.

Built with **Tauri 2**, **React**, **TypeScript**, **Vite**, and **Rust**.

## What it does

- **Issues Home** — detects common React/Node Linux problems and offers 1–3 click fixes
- **System monitors** — processes, ports, CPU, memory
- **Node doctor** — active version, install method, PATH, npm permissions

### Wave 1 fixes

1. Free occupied ports
2. Node version mismatch (nvm/fnm switch)
3. Node/npm PATH repair
4. npm EACCES / permissions repair
5. Clean dependency reinstall
6. Node heap preset (`.env.development.local`)
7. inotify file-watcher limit
8. Safe developer cache cleanup

## Develop

```bash
npm install
npm run tauri dev
```

If system Tauri `-dev` packages are not installed:

```bash
source ~/.local/tauri-deps/env.sh
source ~/.cargo/env
```

## Production build / installer

```bash
npm run installer
```

Creates a Debian package under `installer/` with **login autostart** enabled
(`/etc/xdg/autostart/DevMonitor.desktop`), so DevMonitor starts again after
reboot and power off/on once you log in.

```bash
cd installer
./install.sh
# or: sudo apt install -y ./DevMonitor_0.1.0_amd64.deb
```

Artifacts:

- Installer folder: `installer/DevMonitor_0.1.0_amd64.deb`
- Binary: `src-tauri/target/release/devmonitor`
- Deb (build tree): `src-tauri/target/release/bundle/deb/DevMonitor_0.1.0_amd64.deb`

## Snap Store

Packaging lives in `snap/`. Name: **`devmonitor`**.

```bash
sudo snap install snapcraft --classic
sudo snap install lxd && sudo lxd init --auto
bash scripts/publish-snap.sh --all   # login → register → build → upload edge
```

See [installer/SNAP_STORE.md](installer/SNAP_STORE.md) for classic-confinement approval and stable release steps.
