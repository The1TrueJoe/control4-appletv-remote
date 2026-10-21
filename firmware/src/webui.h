/* Tiny HTTP server: provisioning form, config, and /health. */
#ifndef BRIDGE_WEBUI_H_
#define BRIDGE_WEBUI_H_

#include <stdint.h>

#define WEBUI_PORT 80

/* Start the HTTP server. Serves the config/provisioning page, POST /save, and
 * GET /health (JSON the Control4 driver reads for room/model/id). */
void webui_start(void);

#endif /* BRIDGE_WEBUI_H_ */
