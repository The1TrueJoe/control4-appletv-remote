/* Advertise _appletv-bridge._tcp over mDNS so the Control4 driver finds us. */
#ifndef BRIDGE_MDNS_ADV_H_
#define BRIDGE_MDNS_ADV_H_

/* Register the service on the given port with TXT per docs/FORWARDER.md. */
void mdns_adv_start(uint16_t http_port);

#endif /* BRIDGE_MDNS_ADV_H_ */
