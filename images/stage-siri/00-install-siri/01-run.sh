#!/bin/bash -e
# Install the siri-c4d bridge into the image rootfs and enable it at boot.
#
# Binaries must be present at files/bin/{siri-c4d,siri-events} before pi-gen
# runs this stage — images/build.sh copies the release binaries there.

here="$(dirname "$0")"

if [ ! -f "${here}/files/bin/siri-c4d" ]; then
	echo "ERROR: ${here}/files/bin/siri-c4d missing — build.sh must stage the aarch64 binaries first" >&2
	exit 1
fi

install -Dm755 "${here}/files/bin/siri-c4d"   "${ROOTFS_DIR}/usr/local/bin/siri-c4d"
if [ -f "${here}/files/bin/siri-events" ]; then
	install -Dm755 "${here}/files/bin/siri-events" "${ROOTFS_DIR}/usr/local/bin/siri-events"
fi

install -Dm644 "${here}/files/siri-c4d.service" "${ROOTFS_DIR}/etc/systemd/system/siri-c4d.service"

install -d -m 0755 "${ROOTFS_DIR}/etc/siri-c4d" \
	"${ROOTFS_DIR}/var/lib/siri-c4d" "${ROOTFS_DIR}/var/lib/siri-c4d/audio"
install -Dm640 "${here}/files/config.toml" "${ROOTFS_DIR}/etc/siri-c4d/config.toml"

# Enable the service. The daemon advertises its own mDNS via mdns-sd, so no
# avahi-daemon (it would contend for UDP 5353).
on_chroot <<'CHROOT'
systemctl enable siri-c4d.service
CHROOT
