/*
 * WiFi provisioning for the bridge.
 *
 * UNVERIFIED (no board/SDK here): the exact Zephyr WiFi mgmt API shape varies
 * by version. This uses net_mgmt(NET_REQUEST_WIFI_CONNECT / ..._AP_ENABLE)
 * with struct wifi_connect_req_params; adjust field names to your Zephyr
 * release. The captive form is served by webui.c on the AP interface.
 */
#include "wifi_prov.h"
#include "config.h"

#include <zephyr/kernel.h>
#include <zephyr/logging/log.h>
#include <zephyr/net/net_if.h>
#include <zephyr/net/net_mgmt.h>
#include <zephyr/net/wifi_mgmt.h>
#include <string.h>

LOG_MODULE_REGISTER(wifi_prov, LOG_LEVEL_INF);

static bool g_connected;
static bool g_ap;
static struct net_mgmt_event_callback g_cb;

static void wifi_evt(struct net_mgmt_event_callback *cb, uint32_t evt,
		     struct net_if *iface)
{
	ARG_UNUSED(cb);
	ARG_UNUSED(iface);
	switch (evt) {
	case NET_EVENT_WIFI_CONNECT_RESULT:
		g_connected = true;
		LOG_INF("WiFi connected");
		break;
	case NET_EVENT_WIFI_DISCONNECT_RESULT:
		g_connected = false;
		LOG_WRN("WiFi disconnected");
		break;
	default:
		break;
	}
}

static int sta_connect(const struct bridge_config *cfg)
{
	struct net_if *iface = net_if_get_default();
	struct wifi_connect_req_params p = {0};
	p.ssid = (const uint8_t *)cfg->wifi_ssid;
	p.ssid_length = strlen(cfg->wifi_ssid);
	p.psk = (const uint8_t *)cfg->wifi_psk;
	p.psk_length = strlen(cfg->wifi_psk);
	p.security = p.psk_length ? WIFI_SECURITY_TYPE_PSK : WIFI_SECURITY_TYPE_NONE;
	p.channel = WIFI_CHANNEL_ANY;
	p.band = WIFI_FREQ_BAND_2_4_GHZ;
	return net_mgmt(NET_REQUEST_WIFI_CONNECT, iface, &p, sizeof(p));
}

static int ap_start(void)
{
	struct net_if *iface = net_if_get_default();
	struct wifi_connect_req_params p = {0};
	/* SSID: SiriBridge-XXXX (append part of the MAC in a real build). */
	static char ssid[32];
	snprintk(ssid, sizeof(ssid), "%s-setup", CONFIG_SIRI_BRIDGE_AP_SSID);
	p.ssid = (const uint8_t *)ssid;
	p.ssid_length = strlen(ssid);
	p.security = WIFI_SECURITY_TYPE_NONE; /* open; the form is on 192.168.x.1 */
	p.channel = 1;
	p.band = WIFI_FREQ_BAND_2_4_GHZ;
	g_ap = true;
	LOG_INF("provisioning AP '%s' up; browse http://192.168.4.1/", ssid);
	/* TODO: NET_REQUEST_WIFI_AP_ENABLE signature differs across versions. */
	return net_mgmt(NET_REQUEST_WIFI_AP_ENABLE, iface, &p, sizeof(p));
}

bool wifi_prov_start(void)
{
	net_mgmt_init_event_callback(&g_cb, wifi_evt,
				     NET_EVENT_WIFI_CONNECT_RESULT |
					     NET_EVENT_WIFI_DISCONNECT_RESULT);
	net_mgmt_add_event_callback(&g_cb);

	struct bridge_config *cfg = bridge_config();
	if (cfg->provisioned && cfg->wifi_ssid[0]) {
		if (sta_connect(cfg) == 0) {
			/* Wait briefly for association. */
			for (int i = 0; i < 100 && !g_connected; i++) {
				k_msleep(100);
			}
			if (g_connected) {
				return true;
			}
		}
		LOG_WRN("STA connect failed; falling back to provisioning AP");
	}
	ap_start();
	return false;
}

bool wifi_prov_connected(void)
{
	return g_connected;
}

bool wifi_prov_ap_active(void)
{
	return g_ap;
}
