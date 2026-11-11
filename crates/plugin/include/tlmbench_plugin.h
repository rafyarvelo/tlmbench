/* The C interface of a TelemetryBench native decoder plugin (ABI version 1).
 *
 * A native plugin is a shared library (.so, .dylib, .dll) that exports the functions below. It runs inside the app
 * with the user's own rights: no sandbox. Use a WASM plugin unless you need native speed.
 *
 * Rules: strings are UTF-8 and not NUL-terminated; every pointer you hand out stays valid until the next call on the
 * same session or until tlm_close; `tlm_sniff` and `tlm_describe` may be called from several threads at once; a
 * session is used by one thread at a time. Never let a panic or exception cross these functions. Columns and
 * times are little-endian. */
#ifndef TLMBENCH_PLUGIN_H
#define TLMBENCH_PLUGIN_H
#include <stddef.h>
#include <stdint.h>

#define TLM_ABI 1

typedef struct { const char *ptr; size_t len; } tlm_str;

/* dtype: 0 i8, 1 i16, 2 i32, 3 i64, 4 u8, 5 u16, 6 u32, 7 u64, 8 f32, 9 f64 */
typedef struct { tlm_str name; tlm_str unit; uint8_t dtype; } tlm_channel;

/* `files`: name patterns separated by ';' (`*` matches any text), besides what tlm_sniff claims. */
typedef struct { tlm_str name; tlm_str version; tlm_str description; tlm_str files; } tlm_about;

/* `timed`: chunks carry a time per row in nanoseconds, never decreasing; 0 numbers the samples. */
typedef struct { const tlm_channel *channels; size_t n_channels; uint8_t timed; } tlm_schema;

/* `time`: `rows` values (NULL when not timed). `columns`: one pointer per channel, `rows` values of its type each. */
typedef struct { size_t rows; const int64_t *time; const void *const *columns; } tlm_chunk;

uint32_t tlm_abi(void);                                   /* returns TLM_ABI */
void tlm_describe(tlm_about *out);
int32_t tlm_sniff(const uint8_t *head, size_t len, tlm_str file_name);   /* 1 if this plugin decodes the file */
void *tlm_open(tlm_str path, tlm_schema *schema, tlm_str *error);        /* NULL on error, `error` says why */
int32_t tlm_next(void *session, uint32_t max_rows, tlm_chunk *out, tlm_str *error);  /* 1 a chunk, 0 the end, <0 error */
void tlm_close(void *session);

#endif
