#include "lnmai_native.h"
#include <lean/lean.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>

struct LnmaiResult { LnmaiRecord *records; uint64_t count, capacity; uint32_t failed; char *error; };
extern lean_object *lnmai_native_dispatch(uint64_t, uint32_t, uint64_t, uint64_t);

uint64_t lnmai_native_request_word(uint64_t address, uint64_t field) {
    const LnmaiRequest *r = (const LnmaiRequest *)(uintptr_t)address;
    switch (field) { case 0: return (uint64_t)r->time_us; case 1: return r->level_index;
      case 2: return r->event_count; default: return 0; }
}
uint64_t lnmai_native_event_word(uint64_t address, uint64_t index, uint64_t field) {
    const LnmaiRequest *r = (const LnmaiRequest *)(uintptr_t)address;
    if (index >= r->event_count) return 0;
    const LnmaiInputEvent *e = &r->events[index];
    switch (field) { case 0: return (uint64_t)e->time_us; case 1: return e->kind;
      case 2: return e->position; case 3: return e->is_down; default: return 0; }
}
lean_object *lnmai_native_request_text(uint64_t address) {
    const LnmaiRequest *r = (const LnmaiRequest *)(uintptr_t)address;
    return lean_mk_string_from_bytes(r->text ? r->text : "", (size_t)r->text_len);
}
lean_object *lnmai_native_push(uint64_t address, uint64_t tag, uint64_t a, uint64_t b,
        uint64_t c, uint64_t d, uint64_t e, uint64_t f, uint64_t g, uint64_t h) {
    LnmaiResult *r = (LnmaiResult *)(uintptr_t)address;
    if (!r->failed && r->count == r->capacity) {
        uint64_t capacity = r->capacity ? r->capacity * 2 : 128;
        if (capacity < r->capacity || capacity > SIZE_MAX / sizeof(LnmaiRecord)) r->failed = 1;
        else {
            void *p = realloc(r->records, (size_t)capacity * sizeof(LnmaiRecord));
            if (!p) r->failed = 1;
            else { r->records = p; r->capacity = capacity; }
        }
    }
    if (!r->failed) r->records[r->count++] = (LnmaiRecord){tag, {a,b,c,d,e,f,g,h}};
    return lean_io_result_mk_ok(lean_box(0));
}
lean_object *lnmai_native_output_ok(uint64_t address) {
    return lean_io_result_mk_ok(lean_box(!((LnmaiResult *)(uintptr_t)address)->failed));
}
lean_object *lnmai_native_set_error(uint64_t address, lean_object *message) {
    LnmaiResult *r = (LnmaiResult *)(uintptr_t)address;
    size_t n = lean_string_size(message);
    r->error = malloc(n);
    if (r->error) memcpy(r->error, lean_string_cstr(message), n);
    lean_dec(message);
    return lean_io_result_mk_ok(lean_box(0));
}
uint32_t lnmai_v3_call(uint64_t handle, uint32_t operation, const LnmaiRequest *request, LnmaiResult **out) {
    if (!out) return 1;
    *out = NULL;
    if (!request || request->abi_version != 3 || request->struct_size != sizeof(LnmaiRequest) ||
        request->reserved || operation > 7 || (operation == 3 && request->time_us < 0) ||
        (request->event_count && !request->events) || (request->text_len && !request->text) ||
        request->event_count > SIZE_MAX / sizeof(LnmaiInputEvent) || request->text_len > SIZE_MAX) return 1;
    for (uint64_t i = 0; i < request->event_count; ++i) {
        const LnmaiInputEvent *e = &request->events[i];
        if (e->reserved || e->kind > 3 || e->position >= (e->kind < 2 ? 8 : 33) || e->is_down > 1) return 1;
    }
    LnmaiResult *r = calloc(1, sizeof(*r));
    if (!r) return 2;
    *out = r;
    lean_object *io = lnmai_native_dispatch(handle, operation, (uint64_t)(uintptr_t)request, (uint64_t)(uintptr_t)r);
    if (lean_io_result_is_error(io)) { lean_dec(io); return 3; }
    uint32_t status = lean_unbox_uint32(lean_io_result_get_value(io));
    lean_dec(io);
    return r->failed ? 2 : status;
}
const LnmaiRecord *lnmai_v3_records(const LnmaiResult *r, uint64_t *count) { *count = r->count; return r->records; }
const char *lnmai_v3_error(const LnmaiResult *r) { return r && r->error ? r->error : "native FFI call failed"; }
void lnmai_v3_free_result(LnmaiResult *r) { if (r) { free(r->records); free(r->error); free(r); } }
