# Headless first-boot drop-ins

The baked image is a standard Raspberry Pi OS arm64 image with `siri-c4d`
preinstalled and enabled, so you provision WiFi/hostname/SSH the normal way:

- **Easiest:** flash with **Raspberry Pi Imager** and use its gear/"edit
  settings" to set WiFi, hostname, SSH and user. It injects `custom.toml` for
  you. Nothing here needed.
- **Manual (Bookworm+):** copy `custom.toml.sample` → the card's boot partition
  as `custom.toml`, edit WiFi/hostname/password.
- **Manual (older OS):** copy `wpa_supplicant.conf.sample` → boot partition as
  `wpa_supplicant.conf`, and add an empty file named `ssh`.

After it boots and joins WiFi, find it at `http://siri-c4d.local:8752` (mDNS)
or by IP, and finish setup in the web UI. The Control4 driver also discovers it
over mDNS (`_appletv-bridge._tcp`).
