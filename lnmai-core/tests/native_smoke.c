#include "lnmai_ffi.h"
#include "lnmai_native.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

_Static_assert(sizeof(LnmaiInputEvent) == 24, "input layout");
_Static_assert(sizeof(LnmaiRequest) == 56, "request layout");
_Static_assert(offsetof(LnmaiRequest, events) == 24, "events offset");
_Static_assert(sizeof(LnmaiRecord) == 72, "record layout");

static void check_json(char *json) {
    assert(json && strstr(json, "\"ok\":true"));
    lnmai_string_free(json);
}

static char *payload(char *json) {
    assert(json && strstr(json, "\"ok\":true"));
    const char *start = strstr(json, "\"result\":") + 9;
    size_t length = strlen(start) - 1;
    char *copy = malloc(length + 1);
    assert(copy);
    memcpy(copy, start, length);
    copy[length] = 0;
    lnmai_string_free(json);
    return copy;
}

static uint64_t json_handle(char *json) {
    assert(json && strstr(json, "\"ok\":true"));
    const char *start = strstr(json, "\"handle\":\"");
    assert(start);
    uint64_t h = strtoull(start + 10, NULL, 10);
    lnmai_string_free(json);
    return h;
}

int main(void) {
    const char *chart = "&first=0\n&inote_1=(120)1/2h[4:1]/3-5[4:1],";
    const char *batch = "{\"currentTime\":0,\"events\":[]}";
    assert(lnmai_abi_version() == 2);
    check_json(lnmai_ffi_version_json());
    check_json(lnmai_parse_frontend_chart_json(chart, 1));
    check_json(lnmai_parse_frontend_chart_mode_json(chart, 1, 1));
    check_json(lnmai_parse_frontend_inspection_chart_json(chart, 1));
    check_json(lnmai_parse_lowered_chart_mode_json(chart, 1, 1));
    check_json(lnmai_parse_normalized_chart_json(chart, 1));
    check_json(lnmai_parse_frontend_semantic_chart_json(chart, 1));
    char *spec = payload(lnmai_parse_lowered_chart_json(chart, 1));
    check_json(lnmai_default_tactic_from_chart_json(spec));
    char *state = payload(lnmai_build_game_state_json(spec));
    check_json(lnmai_step_game_state_json(state, batch));
    free(state);
    uint64_t h = json_handle(lnmai_create_game_state_handle(spec));
    check_json(lnmai_get_lowered_chart_json_by_handle(h));
    check_json(lnmai_get_game_state_json_by_handle(h));
    check_json(lnmai_step_game_state_handle_light(h, batch));
    check_json(lnmai_step_game_state_handle(h, batch));
    check_json(lnmai_free_game_state_handle(h));
    h = json_handle(lnmai_create_empty_session_handle());
    check_json(lnmai_load_chart_into_session_from_text(h, chart, 1));
    check_json(lnmai_unload_chart_from_session(h));
    check_json(lnmai_load_chart_into_session_from_json(h, spec));
    check_json(lnmai_free_game_state_handle(h));
    free(spec);
    lnmai_string_free(NULL);

    LnmaiRequest request = {0};
    request.abi_version = 3;
    request.struct_size = sizeof request;
    LnmaiResult *result = NULL;
    uint64_t count = 0;
    assert(lnmai_v3_call(0, 0, &request, &result) == 0);
    const LnmaiRecord *records = lnmai_v3_records(result, &count);
    assert(count == 1 && records && records[0].tag == 0);
    h = records[0].words[0];
    lnmai_v3_free_result(result);
    request.text = chart; request.text_len = strlen(chart); request.level_index = 1;
    assert(lnmai_v3_call(h, 1, &request, &result) == 0);
    lnmai_v3_free_result(result);
    request.text = NULL; request.text_len = 0;
    for (uint32_t operation = 2; operation <= 7; ++operation) {
        assert(lnmai_v3_call(h, operation, &request, &result) == 0);
        (void) lnmai_v3_records(result, &count);
        lnmai_v3_free_result(result);
    }
    assert(lnmai_v3_call(h, 2, &request, &result) == 1);
    assert(strstr(lnmai_v3_error(result), "handle"));
    lnmai_v3_free_result(result);
    request.abi_version = 99;
    assert(lnmai_v3_call(h, 0, &request, &result) == 1 && result == NULL);
    lnmai_v3_free_result(NULL);
    puts("C ABI layouts, all exported entry points, session lifecycle and ownership passed");
}
