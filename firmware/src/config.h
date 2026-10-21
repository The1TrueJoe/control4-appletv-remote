/*
 * Persistent bridge configuration (NVS via the settings subsystem):
 * WiFi creds, the Control4 target, the room, and button-map overrides.
 */
#ifndef BRIDGE_CONFIG_H_
#define BRIDGE_CONFIG_H_

#include <stdbool.h>
#include <stdint.h>

#define CFG_STR 64

struct bridge_config {
	char wifi_ssid[CFG_STR];
	char wifi_psk[CFG_STR];
	/* Control4 target base URL, e.g. "http://10.0.0.20:5080" (webhook) or the
	 * native driver's listen URL. Command API per docs/FORWARDER.md. */
	char target_url[128];
	char target_pass[CFG_STR]; /* optional Bearer/X-API-Key */
	char room[CFG_STR];        /* room id/name; empty = let the target infer */
	char remote_addr[32];      /* the Siri Remote BD address */
	bool provisioned;          /* true once WiFi creds are stored */
};

/* Load from NVS (settings). Fills defaults if unset. */
void bridge_config_load(struct bridge_config *out);

/* Persist the whole config. */
int bridge_config_save(const struct bridge_config *cfg);

/* The live, in-RAM config (loaded at boot). */
struct bridge_config *bridge_config(void);

#endif /* BRIDGE_CONFIG_H_ */
