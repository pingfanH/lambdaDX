#ifndef LNMAI_NATIVE_H
#define LNMAI_NATIVE_H
#include <stdint.h>
#include <stddef.h>
/* ABI 3: borrowed inputs, bridge-owned tagged records, no Lean layouts or JSON.
 * Signed fields use two's-complement bits in record words. See native.rs. */
typedef struct { int64_t time_us; uint32_t kind, position, is_down, reserved; } LnmaiInputEvent;
typedef struct {
    uint32_t abi_version, struct_size;
    int64_t time_us;
    uint32_t level_index, reserved;
    const LnmaiInputEvent *events;
    uint64_t event_count;
    const char *text;
    uint64_t text_len;
} LnmaiRequest;
typedef struct { uint64_t tag, words[8]; } LnmaiRecord;
typedef struct LnmaiResult LnmaiResult;
/* Operations: create=0, load text=1, step=2, seek=3, autoplay=4, chart=5,
 * bindings=6, free session=7. Calls on a session are serialized by the core. */
uint32_t lnmai_v3_call(uint64_t handle, uint32_t operation, const LnmaiRequest *, LnmaiResult **);
const LnmaiRecord *lnmai_v3_records(const LnmaiResult *, uint64_t *count);
const char *lnmai_v3_error(const LnmaiResult *);
void lnmai_v3_free_result(LnmaiResult *);
#endif
