/* Map remote controls to Control4 commands and POST them to the target. */
#ifndef BRIDGE_FORWARDER_H_
#define BRIDGE_FORWARDER_H_

#include <stdbool.h>
#include <stdint.h>

/* A control fired by the remote (button bit or swipe name). `down` is the edge
 * (true=press). The forwarder applies the default map (docs/FORWARDER.md),
 * resolving hold-style actions on press/release, and POSTs to the configured
 * Control4 target. */
void forwarder_button(uint8_t mask, bool down);
void forwarder_swipe(const char *swipe_name); /* "SWIPE_UP" etc. */

#endif /* BRIDGE_FORWARDER_H_ */
