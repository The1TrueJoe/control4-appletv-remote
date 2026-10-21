#include "config.h"

#include <zephyr/settings/settings.h>
#include <zephyr/logging/log.h>
#include <string.h>

LOG_MODULE_REGISTER(bridge_config, LOG_LEVEL_INF);

static struct bridge_config g_cfg;

/* One settings key holding the whole struct - simple and atomic enough here. */
#define CFG_KEY "bridge/cfg"

static int cfg_set(const char *name, size_t len, settings_read_cb read_cb, void *cb_arg)
{
	if (settings_name_next(name, NULL) == 0) {
		ssize_t n = read_cb(cb_arg, &g_cfg, sizeof(g_cfg));
		if (n < 0) {
			return (int)n;
		}
	}
	return 0;
}

SETTINGS_STATIC_HANDLER_DEFINE(bridge, "bridge", NULL, cfg_set, NULL, NULL);

void bridge_config_load(struct bridge_config *out)
{
	memset(&g_cfg, 0, sizeof(g_cfg));
	/* settings_subsys_init()/settings_load() are called once in main. */
	settings_load_subtree("bridge");
	if (out) {
		*out = g_cfg;
	}
}

int bridge_config_save(const struct bridge_config *cfg)
{
	if (cfg && cfg != &g_cfg) {
		g_cfg = *cfg;
	}
	return settings_save_one(CFG_KEY, &g_cfg, sizeof(g_cfg));
}

struct bridge_config *bridge_config(void)
{
	return &g_cfg;
}
