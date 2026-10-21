/*
 * mDNS / DNS-SD advertising of the bridge.
 *
 * UNVERIFIED: Zephyr's DNS-SD registration (DNS_SD_REGISTER_TCP_SERVICE) takes
 * a STATIC instance/txt at build time, while our TXT (room, id, model) is
 * runtime config. Options at integration: (a) register with a fixed instance
 * and minimal TXT and let the driver read room/model from the bridge's HTTP
 * /health, or (b) use a Zephyr version with runtime DNS-SD record APIs.
 * This file takes option (a): advertise name+port+path via DNS-SD; the driver
 * fetches the rest over HTTP. TXT keys still documented in docs/FORWARDER.md.
 */
#include "mdns_adv.h"
#include "config.h"

#include <zephyr/kernel.h>
#include <zephyr/logging/log.h>
#include <zephyr/net/dns_sd.h>
#include <stdint.h>

LOG_MODULE_REGISTER(mdns_adv, LOG_LEVEL_INF);

/* Port is advertised via this variable (DNS-SD reads &port). The HTTP port is
 * fixed at build for the macro; keep it in sync with webui.c (80). */
static uint16_t g_port = 80;

/* Minimal TXT: the driver reads room/model/id from HTTP /health. */
DNS_SD_REGISTER_TCP_SERVICE(bridge_sd, CONFIG_NET_HOSTNAME, "_appletv-bridge",
			    "local", DNS_SD_EMPTY_TXT, &g_port);

void mdns_adv_start(uint16_t http_port)
{
	g_port = http_port;
	LOG_INF("advertising _appletv-bridge._tcp on port %u", http_port);
	/* The DNS-SD responder runs via CONFIG_MDNS_RESPONDER; nothing else to
	 * do here beyond keeping g_port current. */
}
