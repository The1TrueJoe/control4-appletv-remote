/* Voice path: relay the remote's audio to Control4 appled (c4vc). */
#ifndef BRIDGE_VOICE_H_
#define BRIDGE_VOICE_H_

#include <siri_remote.h> /* enum siri_codec */
#include <stddef.h>
#include <stdint.h>

void voice_start(void);
void voice_frame(enum siri_codec codec, const uint8_t *data, size_t len);
void voice_end(void);

#endif /* BRIDGE_VOICE_H_ */
