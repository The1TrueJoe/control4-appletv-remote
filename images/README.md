# Raspberry Pi image

Bakes a flashable **Raspberry Pi OS Lite (arm64)** image with the `siri-c4d`
bridge preinstalled and enabled. The 64-bit image boots on **Pi 3, 4 and 5**,
so there is one image for all three.

## Layout

```
images/
├── config                     pi-gen config (arm64 Lite, stage list, defaults)
├── stage-siri/                custom pi-gen stage
│   ├── prerun.sh              seed rootfs from the previous stage
│   ├── EXPORT_IMAGE           export a flashable image after this stage
│   └── 00-install-siri/
│       ├── 00-packages        bluez, libopus0, libspeex1, libdbus-1-3
│       ├── 01-run.sh          install binaries + unit + config, enable services
│       └── files/             siri-c4d.service, config.toml, bin/ (CI drops binaries)
├── boot-overlay/              optional headless WiFi/hostname drop-ins
└── build.sh                   fetch pi-gen, stage binaries, build
```

## Building

`build.sh` needs the aarch64 binaries (from the release workflow or a local
cross-build with `scripts/Dockerfile.build`) and a pi-gen-capable host:

```bash
images/build.sh --bin-dir path/to/dist   # dist/siri-c4d-aarch64 [, siri-events-aarch64]
```

**Heavy prerequisites** (pi-gen): a Debian/Ubuntu host or its Docker flow,
root/privileged, `qemu-user-static`, and ~10 GB free. CI (`.github/workflows/
release.yml`, on a `v*` tag) runs the Docker flow and uploads the image to the
GitHub Release; because pi-gen is slow and needs privilege, that job is
best-effort (`continue-on-error`) so the binaries / driver / firmware always
publish even if image baking needs a rerun.

## Flashing & first boot

Flash with Raspberry Pi Imager and set WiFi/hostname/SSH in its settings, or use
`boot-overlay/` (see its README). On boot the Pi joins WiFi, starts `siri-c4d`,
and is reachable at `http://siri-c4d.local:8752`.
