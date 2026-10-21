/*
 * HTTP server for provisioning + config + health.
 *
 * UNVERIFIED: Zephyr's HTTP server API (HTTP_SERVICE_DEFINE / HTTP_RESOURCE_*
 * and the http_resource_detail_dynamic handler signature) changed across
 * versions. This is written to the current (http_server) shape; adjust the
 * handler prototype to your Zephyr release. The HTML is intentionally tiny.
 */
#include "webui.h"
#include "config.h"

#include <zephyr/kernel.h>
#include <zephyr/logging/log.h>
#include <zephyr/net/http/server.h>
#include <zephyr/net/http/service.h>
#include <stdio.h>
#include <string.h>

LOG_MODULE_REGISTER(webui, LOG_LEVEL_INF);

static uint16_t http_port = WEBUI_PORT;
HTTP_SERVICE_DEFINE(bridge_http, "0.0.0.0", &http_port, 1, 4, NULL, NULL);

static const char PAGE[] =
	"<!doctype html><meta name=viewport content='width=device-width'>"
	"<title>Siri Bridge</title><h2>Siri Remote Bridge</h2>"
	"<form method=post action=/save>"
	"WiFi SSID <input name=ssid><br>"
	"WiFi password <input name=psk type=password><br>"
	"Control4 URL <input name=url placeholder='http://10.0.0.20:5080'><br>"
	"Control4 password <input name=pass type=password><br>"
	"Room <input name=room placeholder='Living Room'><br>"
	"Remote address <input name=remote placeholder='DC:2B:2A:..'><br>"
	"<button>Save &amp; reboot</button></form>";

/* GET / : the config form. */
static int page_handler(struct http_client_ctx *client, enum http_data_status status,
			const struct http_request_ctx *req, struct http_response_ctx *resp,
			void *user)
{
	ARG_UNUSED(client); ARG_UNUSED(status); ARG_UNUSED(req); ARG_UNUSED(user);
	resp->body = (const uint8_t *)PAGE;
	resp->body_len = sizeof(PAGE) - 1;
	resp->final_chunk = true;
	return 0;
}
static struct http_resource_detail_dynamic page_detail = {
	.common = { .type = HTTP_RESOURCE_TYPE_DYNAMIC,
		    .bitmask_of_supported_http_methods = BIT(HTTP_GET) },
	.cb = page_handler,
};
HTTP_RESOURCE_DEFINE(page_rsrc, bridge_http, "/", &page_detail);

/* GET /health : JSON the Control4 driver reads (room/model/id). */
static int health_handler(struct http_client_ctx *client, enum http_data_status status,
			  const struct http_request_ctx *req, struct http_response_ctx *resp,
			  void *user)
{
	ARG_UNUSED(client); ARG_UNUSED(status); ARG_UNUSED(req); ARG_UNUSED(user);
	static char json[192];
	struct bridge_config *cfg = bridge_config();
	int n = snprintf(json, sizeof(json),
			 "{\"ok\":true,\"name\":\"%s\",\"room\":\"%s\",\"model\":\"a1513\"}",
			 CONFIG_NET_HOSTNAME, cfg->room);
	resp->body = (const uint8_t *)json;
	resp->body_len = n;
	resp->final_chunk = true;
	return 0;
}
static struct http_resource_detail_dynamic health_detail = {
	.common = { .type = HTTP_RESOURCE_TYPE_DYNAMIC,
		    .bitmask_of_supported_http_methods = BIT(HTTP_GET) },
	.cb = health_handler,
};
HTTP_RESOURCE_DEFINE(health_rsrc, bridge_http, "/health", &health_detail);

/* POST /save : parse form fields, persist, reboot. (Parsing TODO.) */
static int save_handler(struct http_client_ctx *client, enum http_data_status status,
			const struct http_request_ctx *req, struct http_response_ctx *resp,
			void *user)
{
	ARG_UNUSED(client); ARG_UNUSED(user);
	/* TODO: accumulate req->data across chunks, urldecode the form fields
	 * (ssid/psk/url/pass/room/remote) into bridge_config(), save, then
	 * sys_reboot(SYS_REBOOT_COLD). */
	if (status == HTTP_SERVER_DATA_FINAL) {
		struct bridge_config *cfg = bridge_config();
		cfg->provisioned = true;
		bridge_config_save(cfg);
		static const char ok[] = "saved; rebooting";
		resp->body = (const uint8_t *)ok;
		resp->body_len = sizeof(ok) - 1;
		resp->final_chunk = true;
		LOG_INF("config saved via web UI");
		/* sys_reboot(SYS_REBOOT_COLD);  // enable once parsing is done */
	}
	return 0;
}
static struct http_resource_detail_dynamic save_detail = {
	.common = { .type = HTTP_RESOURCE_TYPE_DYNAMIC,
		    .bitmask_of_supported_http_methods = BIT(HTTP_POST) },
	.cb = save_handler,
};
HTTP_RESOURCE_DEFINE(save_rsrc, bridge_http, "/save", &save_detail);

void webui_start(void)
{
	http_port = WEBUI_PORT;
	int err = http_server_start();
	if (err) {
		LOG_ERR("http_server_start: %d", err);
	} else {
		LOG_INF("web UI on http://0.0.0.0:%u/", WEBUI_PORT);
	}
}
