#ifndef MUSUBU_H
#define MUSUBU_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct MusubuEngine MusubuEngine;

bool init(MusubuEngine **output);
void uninit(MusubuEngine *engine);

/* Input strings are UTF-8 byte ranges; lengths exclude any NUL terminator.
 * Compilation clears the previous diagnostic, including on success.
 * Use a path relative to your project root for filename. */
bool compile(MusubuEngine *engine, const char *code, size_t length);
bool compile_with_filename(MusubuEngine *engine, const char *code, size_t code_length,
                           const char *filename, size_t filename_length);

/* Borrowed UTF-8 bytes, NOT NUL-terminated. Do not free the returned pointer.
 * Valid until the engine is mutated or destroyed; copy before recompiling.
 * Returns NULL and sets *length to zero if no error exists or engine is NULL.
 * A NULL length pointer returns NULL. Calls on one engine must not overlap. */
const char *get_compile_error(const MusubuEngine *engine, size_t *length);

/* Tags are uint32_t in the ABI, not a compiler-dependent C enum. */
#define MUSUBU_UNIT UINT32_C(0)
#define MUSUBU_BOOL UINT32_C(1)
#define MUSUBU_I8 UINT32_C(2)
#define MUSUBU_I16 UINT32_C(3)
#define MUSUBU_I32 UINT32_C(4)
#define MUSUBU_I64 UINT32_C(5)
#define MUSUBU_U8 UINT32_C(6)
#define MUSUBU_U16 UINT32_C(7)
#define MUSUBU_U32 UINT32_C(8)
#define MUSUBU_U64 UINT32_C(9)
#define MUSUBU_F32 UINT32_C(10)
#define MUSUBU_F64 UINT32_C(11)

/* Initialize all fields (e.g. MusubuValue value = {0}).
 * I8..I64 use signed_value; U8..U64 and BOOL use unsigned_value.
 * BOOL accepts only 0 or 1. F32/F64 use float_value; F32 rounds to float.
 * UNIT has no payload. Other fields are ignored. No allocations to release. */
typedef struct MusubuValue {
    uint32_t kind;
    int64_t signed_value;
    uint64_t unsigned_value;
    double float_value;
} MusubuValue;

/* name is the UTF-8 source function name (e.g. "add"), not NUL-terminated.
 * Requires successful compilation. Arguments must exactly match parameter types;
 * no implicit conversion. Numeric/bool/unit signatures only.
 * arguments may be NULL only when argument_count is zero. output is required.
 * On success, writes caller-owned output (UNIT for no return value).
 * On failure, output is unchanged; inspect get_runtime_error().
 * Each call clears the previous runtime error; compile errors are independent.
 * All non-null pointers must be valid and correctly aligned; input/output must
 * not alias engine storage. Calls on the same engine must not overlap. */
bool call_function(MusubuEngine *engine, const char *name, size_t name_length,
                   const MusubuValue *arguments, size_t argument_count,
                   MusubuValue *output);

/* Borrowed UTF-8 bytes, NOT NUL-terminated. Do not free. Valid until the next
 * call_function or uninit. NULL engine/no error writes zero length and returns
 * NULL. A NULL length pointer returns NULL. Copy before the next call. */
const char *get_runtime_error(const MusubuEngine *engine, size_t *length);

#ifdef __cplusplus
}
#endif

#endif
