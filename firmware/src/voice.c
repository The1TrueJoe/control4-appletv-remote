/*
 * Voice relay to Control4 appled over c4vc.
 *
 * The c4vc BINARY framing below is fully specified in
 * docs/c4-voice-path/PROTOCOL.md and implemented here. What is NOT yet wired
 * (flagged TODO) because it is unproven live and/or heavy:
 *   - The appled :9200 JSON setup (remote -> activate -> button down) that
 *     opens the dynamic c4vc port. Needs the live garage bring-up first.
 *   - A1513 Speex -> Opus transcode (CONFIG_SIRI_BRIDGE_TRANSCODE): appled only
 *     accepts Opus. A1962 relays Opus verbatim (this file's happy path).
 */
#include "voice.h"
#include "config.h"

#include <zephyr/kernel.h>
#include <zephyr/logging/log.h>
#include <zephyr/net/socket.h>
#include <string.h>

LOG_MODULE_REGISTER(voice, LOG_LEVEL_INF);

#define C4VC_PORT_DEFAULT 9201 /* opened dynamically by appled; see PROTOCOL.md */

static int g_sock = -1;
static uint32_t g_stream_id;

/* ---- c4vc binary framing (docs/c4-voice-path/PROTOCOL.md §1,4,6) ---- */

static void put_be16(uint8_t *p, uint16_t v) { p[0] = v >> 8; p[1] = v; }
static void put_be32(uint8_t *p, uint32_t v)
{
	p[0] = v >> 24; p[1] = v >> 16; p[2] = v >> 8; p[3] = v;
}

/* 12-byte handshake: "C4VC" | ver=1 | rsvd=0 | stream_id(!=0), all big-endian. */
static int c4vc_handshake(int sock, uint32_t id)
{
	uint8_t hs[12] = {'C', '4', 'V', 'C'};
	put_be16(&hs[4], 1);
	put_be16(&hs[6], 0);
	put_be32(&hs[8], id);
	if (zsock_send(sock, hs, sizeof(hs), 0) != (ssize_t)sizeof(hs)) {
		return -EIO;
	}
	uint8_t reply[12];
	ssize_t n = zsock_recv(sock, reply, sizeof(reply), 0);
	if (n != (ssize_t)sizeof(reply) || memcmp(reply, "C4VC", 4) != 0) {
		return -EPROTO;
	}
	uint32_t echoed = ((uint32_t)reply[8] << 24) | ((uint32_t)reply[9] << 16) |
			  ((uint32_t)reply[10] << 8) | reply[11];
	return (echoed == id) ? 0 : -EACCES; /* appled zeroes the id to reject */
}

/* Audio packet: 0x01 | 0x00 | len(be16)=12+N | ms(be32) | rms(f64 be) | opus. */
static int c4vc_send_opus(int sock, const uint8_t *opus, size_t n, uint32_t ms, double db)
{
	uint8_t hdr[16] = {0x01, 0x00};
	put_be16(&hdr[2], (uint16_t)(12 + n));
	put_be32(&hdr[4], ms);
	uint64_t bits;
	memcpy(&bits, &db, sizeof(bits)); /* IEEE-754; appled reads big-endian */
	for (int i = 0; i < 8; i++) {
		hdr[8 + i] = (uint8_t)(bits >> (56 - 8 * i));
	}
	if (zsock_send(sock, hdr, sizeof(hdr), 0) != (ssize_t)sizeof(hdr)) {
		return -EIO;
	}
	return (zsock_send(sock, opus, n, 0) == (ssize_t)n) ? 0 : -EIO;
}

/* Finish: 0x02 | success | 0x00 0x00. */
static void c4vc_finish(int sock, bool ok)
{
	uint8_t f[4] = {0x02, ok ? 1 : 0, 0, 0};
	(void)zsock_send(sock, f, sizeof(f), 0);
}

/* ---- public ---- */

void voice_start(void)
{
	/* TODO: speak appled :9200 JSON (remote/activate/button down) to open the
	 * c4vc port, then connect it here. Gated on the live bring-up. */
	g_stream_id = (uint32_t)(sys_rand32_get() | 1);
	LOG_INF("voice start (c4vc stream id %u) - appled session setup TODO", g_stream_id);
	g_sock = -1; /* not opened until the :9200 setup lands */
}

void voice_frame(enum siri_codec codec, const uint8_t *data, size_t len)
{
	if (codec == SIRI_CODEC_SPEEX) {
#ifdef CONFIG_SIRI_BRIDGE_TRANSCODE
		/* TODO: libspeex decode -> libopus encode (16 kHz mono), then send
		 * the Opus. Heavy; S3 only. */
		LOG_WRN("A1513 Speex transcode not yet implemented");
#else
		LOG_WRN("A1513 Speex dropped (enable SIRI_BRIDGE_TRANSCODE on an S3)");
#endif
		return;
	}
	/* A1962: relay Opus verbatim. */
	if (g_sock >= 0) {
		(void)c4vc_send_opus(g_sock, data, len, 20 /*ms*/, -40.0 /*db est*/);
	}
}

void voice_end(void)
{
	if (g_sock >= 0) {
		c4vc_finish(g_sock, true);
		zsock_close(g_sock);
		g_sock = -1;
	}
}
