/*
 * Siri Remote -> Control4 bridge firmware (ESP32-S3, Zephyr).
 *
 * Boot flow: load config -> bring up WiFi (STA, or provisioning AP) -> start
 * the web UI -> once on the network, advertise over mDNS and start the BLE
 * central. Remote buttons/swipes are POSTed to the Control4 target; Siri voice
 * goes to appled. The module (siri_remote) owns all BLE/protocol detail.
 */
#include <zephyr/kernel.h>
#include <zephyr/logging/log.h>
#include <zephyr/settings/settings.h>

#include <siri_remote.h>

#include "config.h"
#include "forwarder.h"
#include "mdns_adv.h"
#include "voice.h"
#include "webui.h"
#include "wifi_prov.h"

LOG_MODULE_REGISTER(main, LOG_LEVEL_INF);

/* --- remote event callbacks: fan out to forwarder + voice --- */

static void on_button(uint8_t mask, bool down)
{
	/* SIRI is the voice path (handled by on_voice_*); everything else maps
	 * to a Control4 command. */
	if (mask != SIRI_BTN_SIRI) {
		forwarder_button(mask, down);
	}
}

static void on_swipe(enum siri_swipe dir)
{
	static const char *n[] = {"SWIPE_UP", "SWIPE_DOWN", "SWIPE_LEFT", "SWIPE_RIGHT"};
	forwarder_swipe(n[dir]);
}

static void on_voice_start(void) { voice_start(); }
static void on_voice_frame(enum siri_codec c, const uint8_t *d, size_t n)
{
	voice_frame(c, d, n);
}
static void on_voice_end(const struct siri_voice_stats *st)
{
	ARG_UNUSED(st);
	voice_end();
}
static void on_connected(uint16_t mtu, enum siri_model model)
{
	LOG_INF("remote connected (mtu %u, model %d)", mtu, model);
}

static const struct siri_remote_cb remote_cb = {
	.on_connected = on_connected,
	.on_button = on_button,
	.on_swipe = on_swipe,
	.on_voice_start = on_voice_start,
	.on_voice_frame = on_voice_frame,
	.on_voice_end = on_voice_end,
};

int main(void)
{
	settings_subsys_init();
	settings_load();
	bridge_config_load(NULL);
	struct bridge_config *cfg = bridge_config();

	webui_start(); /* available on both AP (provisioning) and STA */

	bool online = wifi_prov_start();
	if (!online) {
		LOG_WRN("not on the network yet - finish provisioning at the AP");
		return 0; /* stay up serving the provisioning form */
	}

	mdns_adv_start(WEBUI_PORT);

	if (cfg->remote_addr[0]) {
		struct siri_remote_config rc = {
			.address = cfg->remote_addr,
			.addr_public = true,
			.cb = &remote_cb,
		};
		int err = siri_remote_start(&rc);
		if (err) {
			LOG_ERR("siri_remote_start: %d", err);
		}
	} else {
		LOG_WRN("no remote address configured yet (set it in the web UI)");
	}
	return 0;
}
