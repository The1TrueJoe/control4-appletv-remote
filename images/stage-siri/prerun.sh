#!/bin/bash -e
# Standard pi-gen stage prerun: seed this stage's rootfs from the previous one.
if [ ! -d "${ROOTFS_DIR}" ]; then
	copy_previous
fi
