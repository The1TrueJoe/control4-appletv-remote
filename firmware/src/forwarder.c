/*
 * Forwarder: remote control -> Control4 command, POSTed to the target per
 * docs/FORWARDER.md. Default map matches siri-c4d's config.rs defaults.
 *
 * UNVERIFIED: Zephyr's HTTP client API (http_client_req / struct
 * http_request) field names vary by version; the POST helper is written to
 * that shape but review on your release.
 */
#include "forwarder.h"
#include "config.h"

#include <siri_remote.h> /* SIRI_BTN_* */

#include <zephyr/kernel.h>
#include <zephyr/logging/log.h>
#include <zephyr/net/http/client.h>
#include <zephyr/net/socket.h>
#include <stdio.h>
#include <string.h>

LOG_MODULE_REGISTER(forwarder, LOG_LEVEL_INF);

struct mapping {
	uint8_t mask;
	const char *path;    /* "/command" or "/preset" ... */
	const char *press;   /* command/body on press */
	const char *release; /* command on release (hold), or NULL */
};

/* Default button map (docs/FORWARDER.md). Hold entries set release. */
static const struct mapping BTN_MAP[] = {
	{SIRI_BTN_MENU,       "/command", "MENU", NULL},
	{SIRI_BTN_PLAY_PAUSE, "/command", "PLAYPAUSE", NULL},
	{SIRI_BTN_VOL_UP,     "/command", "START_VOL_UP", "STOP_VOL_UP"},
	{SIRI_BTN_VOL_DOWN,   "/command", "START_VOL_DOWN", "STOP_VOL_DOWN"},
	{SIRI_BTN_TOUCHPAD,   "/command", "ENTER", NULL},
	{SIRI_BTN_TOUCHPAD_2, "/command", "BACK", NULL},
	{SIRI_BTN_AIRPLAY,    "/command", "ROOM_OFF", NULL},
	/* SIRI handled by the voice path, not here. */
};

/* Fire one command: POST {"command":CMD[,"room":ROOM]} to target_url+path. */
static void post_command(const char *path, const char *command)
{
	struct bridge_config *cfg = bridge_config();
	if (!cfg->target_url[0]) {
		LOG_WRN("no Control4 target configured");
		return;
	}
	char body[128];
	if (cfg->room[0]) {
		snprintf(body, sizeof(body),
			 "{\"command\":\"%s\",\"room\":\"%s\"}", command, cfg->room);
	} else {
		snprintf(body, sizeof(body), "{\"command\":\"%s\"}", command);
	}

	/* TODO: parse target_url into host/port once; connect; http_client_req.
	 * Sketch of the Zephyr HTTP client call: */
	LOG_INF("POST %s%s %s", cfg->target_url, path, body);
	/*
	 * int sock = connect_to(cfg->target_url);
	 * struct http_request req = {
	 *     .method = HTTP_POST, .url = path, .host = host,
	 *     .protocol = "HTTP/1.1", .payload = body, .payload_len = strlen(body),
	 *     .header_fields = (const char *[]){
	 *         "Content-Type: application/json\r\n",
	 *         cfg->target_pass[0] ? bearer_header(cfg->target_pass) : NULL, NULL },
	 *     .response = resp_cb, .recv_buf = buf, .recv_buf_len = sizeof(buf),
	 * };
	 * http_client_req(sock, &req, 2000, NULL);
	 * close(sock);
	 */
}

void forwarder_button(uint8_t mask, bool down)
{
	for (size_t i = 0; i < ARRAY_SIZE(BTN_MAP); i++) {
		if (BTN_MAP[i].mask != mask) {
			continue;
		}
		if (down) {
			post_command(BTN_MAP[i].path, BTN_MAP[i].press);
		} else if (BTN_MAP[i].release) {
			post_command(BTN_MAP[i].path, BTN_MAP[i].release);
		}
		return;
	}
}

void forwarder_swipe(const char *swipe_name)
{
	/* SWIPE_UP/DOWN/LEFT/RIGHT -> UP/DOWN/LEFT/RIGHT (docs/FORWARDER.md). */
	const char *cmd = swipe_name + 6; /* skip "SWIPE_" */
	post_command("/command", cmd);
}
