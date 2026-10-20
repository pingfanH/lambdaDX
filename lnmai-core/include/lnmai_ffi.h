/*
 * lnmai-core Rust FFI (C ABI).
 *
 * Every function returns a heap-allocated, NUL-terminated JSON envelope:
 *
 *   {"ok":true,"result":...}
 *   {"ok":false,"error":{"code":"...","message":"..."}}
 *
 * Release every returned pointer with `lnmai_string_free`.
 *
 * The Lean build exports the same symbol names but marshals Lean `String`
 * objects; this Rust build uses plain C strings.
 */
#ifndef LNMAI_FFI_H
#define LNMAI_FFI_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

uint64_t lnmai_abi_version(void);
char *lnmai_ffi_version_json(void);

/* Parse APIs (chart text + level index -> JSON IR). */
char *lnmai_parse_frontend_chart_json(const char *content, uint32_t level_index);
char *lnmai_parse_frontend_chart_mode_json(const char *content, uint32_t level_index, uint8_t is_classic);
char *lnmai_parse_frontend_inspection_chart_json(const char *content, uint32_t level_index);
char *lnmai_parse_lowered_chart_json(const char *content, uint32_t level_index);
char *lnmai_parse_lowered_chart_mode_json(const char *content, uint32_t level_index, uint8_t is_classic);
char *lnmai_parse_normalized_chart_json(const char *content, uint32_t level_index);
char *lnmai_parse_frontend_semantic_chart_json(const char *content, uint32_t level_index);

/* Default autoplay tactic (ChartSpec JSON -> TimedInputEvent list). */
char *lnmai_default_tactic_from_chart_json(const char *chart_spec_json);

/* Stateless runtime and state restoration. */
char *lnmai_build_game_state_json(const char *chart_spec_json);
char *lnmai_step_game_state_json(const char *state_json, const char *batch_json);
char *lnmai_create_game_state_handle(const char *chart_spec_json);

/* Session runtime APIs (preferred gameplay surface). */
char *lnmai_create_empty_session_handle(void);
char *lnmai_load_chart_into_session_from_text(uint64_t handle, const char *content, uint32_t level_index);
char *lnmai_load_chart_into_session_from_json(uint64_t handle, const char *chart_spec_json);
char *lnmai_unload_chart_from_session(uint64_t handle);
char *lnmai_get_lowered_chart_json_by_handle(uint64_t handle);
char *lnmai_free_game_state_handle(uint64_t handle);
char *lnmai_step_game_state_handle_light(uint64_t handle, const char *batch_json);
char *lnmai_step_game_state_handle(uint64_t handle, const char *batch_json);
char *lnmai_get_game_state_json_by_handle(uint64_t handle);

/* Memory management. */
void lnmai_string_free(char *ptr);

#ifdef __cplusplus
}
#endif

#endif /* LNMAI_FFI_H */
