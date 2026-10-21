/*
 * WiFi bring-up: connect with stored creds, or start a SoftAP + captive form
 * for first-boot provisioning.
 */
#ifndef BRIDGE_WIFI_PROV_H_
#define BRIDGE_WIFI_PROV_H_

#include <stdbool.h>

/* If provisioned, join the stored network (STA). Otherwise (or on repeated
 * STA failure) start the provisioning SoftAP. Returns true if STA connected. */
bool wifi_prov_start(void);

/* True once associated to the configured network with an IP. */
bool wifi_prov_connected(void);

/* True while the provisioning SoftAP is up. */
bool wifi_prov_ap_active(void);

#endif /* BRIDGE_WIFI_PROV_H_ */
