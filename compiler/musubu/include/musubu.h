#ifndef MUSUBU_H
#define MUSUBU_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* The Rust cdylib exports the platform C ABI. Explicit cdecl also supports
 * Windows callers built with a different default calling convention. */
#if defined(_WIN32)
#define MUSUBU_API __declspec(dllimport)
#if defined(_MSC_VER)
#define MUSUBU_CALL __cdecl
#elif defined(__i386__)
#define MUSUBU_CALL __attribute__((cdecl))
#else
#define MUSUBU_CALL
#endif
#else
#define MUSUBU_API
#define MUSUBU_CALL
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef struct MusubuEngine MusubuEngine;

MUSUBU_API bool MUSUBU_CALL init(MusubuEngine **output);
MUSUBU_API void MUSUBU_CALL uninit(MusubuEngine *engine);

/* Per-call limits, reset for each call (including after failure).
 * Defaults: 1,000,000 instructions and 256 frames including the entry function.
 * Both must be positive. Failure leaves settings unchanged; runtime errors apply.
 * Recompilation preserves limits. This is not a total memory/time sandbox. */
MUSUBU_API bool MUSUBU_CALL set_execution_limits(MusubuEngine *engine, uint64_t instructions, size_t call_depth);

/* Input strings are UTF-8 byte ranges; lengths exclude any NUL terminator.
 * Compilation clears the previous diagnostic, including on success.
 * Success replaces the entire program; removed functions no longer exist.
 * Failure preserves the previous program, source metadata and function tokens.
 * Use a path relative to your project root for filename. */
MUSUBU_API bool MUSUBU_CALL compile(MusubuEngine *engine, const char *code, size_t length);
MUSUBU_API bool MUSUBU_CALL compile_with_filename(MusubuEngine *engine, const char *code, size_t code_length,
                           const char *filename, size_t filename_length);

/* Borrowed UTF-8 bytes, NOT NUL-terminated. Do not free the returned pointer.
 * Valid until the engine is mutated or destroyed; copy before recompiling.
 * Returns NULL and sets *length to zero if no error exists or engine is NULL.
 * A NULL length pointer returns NULL. Calls on one engine must not overlap. */
MUSUBU_API const char *MUSUBU_CALL get_compile_error(const MusubuEngine *engine, size_t *length);

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
#define MUSUBU_VECTOR UINT32_C(12)
#define MUSUBU_MATRIX UINT32_C(13)
#define MUSUBU_ENUM UINT32_C(14)
#define MUSUBU_INVALID UINT32_MAX

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
MUSUBU_API bool MUSUBU_CALL call_function(MusubuEngine *engine, const char *name, size_t name_length,
                   const MusubuValue *arguments, size_t argument_count,
                   MusubuValue *output);

/* Borrowed UTF-8 bytes, NOT NUL-terminated. Do not free. Valid until the next
 * API call taking a mutable engine or uninit. NULL engine/no error writes zero length and returns
 * NULL. A NULL length pointer returns NULL. Copy before the next call. */
MUSUBU_API const char *MUSUBU_CALL get_runtime_error(const MusubuEngine *engine, size_t *length);

typedef struct MusubuValueHandle MusubuValueHandle;

/* Program identity includes engine identity and revision. Treat fields as opaque.
 * Tokens remain valid after a failed compile, but not after a successful compile.
 * No allocations to release. Numeric-ID APIs remain available for compatibility. */
typedef struct MusubuFunctionToken {
    uint64_t program;
    size_t function;
} MusubuFunctionToken;

MUSUBU_API bool MUSUBU_CALL get_function_token(MusubuEngine *engine, const char *name,
    size_t name_length, MusubuFunctionToken *output);
MUSUBU_API bool MUSUBU_CALL call_function_value_by_token(MusubuEngine *engine,
    const MusubuFunctionToken *token, const MusubuValueHandle *const *arguments,
    size_t count, MusubuValueHandle **output);

/* Handle API: scalar, enum, vec and mat (including recursively nested enum payloads
 * made of these types). Struct/tuple/array payloads are not yet supported.
 * Existing MusubuValue/call_function remain scalar-only; do not put aggregate
 * tags into MusubuValue. Use handles and call_function_value for aggregate calls.
 *
 * Each successful constructor/call/value_get_element writes a NEW owned handle.
 * Release each with value_free exactly once. Handles are immutable, independent
 * of engines, and remain valid across calls, recompilation and engine destruction.
 * Inputs are copied, never consumed. You may free a parent after extracting a child.
 * Output slots must be writable; release old owned handles before reusing a slot.
 * Failures leave output unchanged. NULL engine/output fails. Each bool-returning
 * API clears the engine's previous runtime error and records a new error on failure.
 * All pointers must be valid and aligned; inputs/output must not alias engine
 * storage. Calls on the same engine must not overlap. Counts are element counts.
 * Null input arrays are allowed only for zero count. Names are UTF-8 byte ranges
 * without NUL terminators. */
MUSUBU_API bool MUSUBU_CALL value_create_scalar(MusubuEngine *engine, const MusubuValue *scalar,
                         MusubuValueHandle **output);

/* Nonempty vector of identically typed numeric components. */
MUSUBU_API bool MUSUBU_CALL value_create_vector(MusubuEngine *engine, const MusubuValue *components,
                         size_t count, MusubuValueHandle **output);

/* Nonempty matrix of equally typed/sized vector columns, in column-major order.
 * value_get_length(matrix) is columns; value_get_length(column) is rows. */
MUSUBU_API bool MUSUBU_CALL value_create_matrix(MusubuEngine *engine, const MusubuValueHandle *const *columns,
                         size_t count, MusubuValueHandle **output);

/* Packed column-major buffers: data[column * rows + row].
 * No padding between columns. For GLM packed matrices use glm::value_ptr(matrix).
 * Constructors COPY columns * rows scalars; input memory is not retained.
 * Copy functions require an exact shape/scalar type match and writable storage
 * for columns * rows scalars. Failure leaves output/buffer unchanged.
 * Dimensions must be nonzero; data must be non-NULL and correctly aligned.
 * Buffers must not alias engine or handle storage. Handle API ownership and
 * runtime diagnostic rules above apply. Aligned/padded matrices must be packed
 * before calling these APIs. Existing APIs and layouts are unchanged. */
MUSUBU_API bool MUSUBU_CALL value_create_matrix_f32(MusubuEngine *engine, const float *data,
                             size_t columns, size_t rows, MusubuValueHandle **output);
MUSUBU_API bool MUSUBU_CALL value_create_matrix_f64(MusubuEngine *engine, const double *data,
                             size_t columns, size_t rows, MusubuValueHandle **output);
MUSUBU_API bool MUSUBU_CALL value_copy_matrix_f32(MusubuEngine *engine, const MusubuValueHandle *value,
                           float *data, size_t columns, size_t rows);
MUSUBU_API bool MUSUBU_CALL value_copy_matrix_f64(MusubuEngine *engine, const MusubuValueHandle *value,
                           double *data, size_t columns, size_t rows);

/* Enum descriptor: type/variant names are matched against the called function's
 * signature at call time. Unknown variants, wrong field count/types are rejected
 * there. Unit variants have zero fields. Tuple and named variant fields must be
 * supplied/read in SOURCE DECLARATION ORDER (not alphabetical order). */
MUSUBU_API bool MUSUBU_CALL value_create_enum(MusubuEngine *engine, const char *type_name, size_t type_length,
                       const char *variant, size_t variant_length,
                       const MusubuValueHandle *const *fields, size_t count,
                       MusubuValueHandle **output);

MUSUBU_API bool MUSUBU_CALL call_function_value(MusubuEngine *engine, const char *name, size_t name_length,
                         const MusubuValueHandle *const *arguments, size_t count,
                         MusubuValueHandle **output);

/* Resolve a source name once, then call without UTF-8 parsing or name lookup.
 * IDs are engine-local size_t values; zero is a valid ID. Resolve again after
 * EVERY compilation attempt. Never reuse an ID with another engine.
 * Stale/foreign IDs are not detected if their numeric value names a function.
 * Lookup does not validate FFI signature support; calls perform the same type
 * and argument checks as call_function_value. Unknown IDs fail.
 * Both functions clear runtime diagnostics; failures leave output unchanged.
 * Existing handle API pointer/ownership rules apply. No ID release is needed. */
MUSUBU_API bool MUSUBU_CALL get_function_id(MusubuEngine *engine, const char *name, size_t name_length,
                     size_t *output);
MUSUBU_API bool MUSUBU_CALL call_function_value_by_id(MusubuEngine *engine, size_t function_id,
                               const MusubuValueHandle *const *arguments, size_t count,
                               MusubuValueHandle **output);

MUSUBU_API bool MUSUBU_CALL value_get_scalar(MusubuEngine *engine, const MusubuValueHandle *value,
                      MusubuValue *output);
MUSUBU_API bool MUSUBU_CALL value_get_element(MusubuEngine *engine, const MusubuValueHandle *value,
                       size_t index, MusubuValueHandle **output);

/* Read-only accessors do not change runtime diagnostics.
 * Null value: kind is MUSUBU_INVALID, length is zero.
 * Scalar length is zero; other lengths are components, columns or fields. */
MUSUBU_API uint32_t MUSUBU_CALL value_get_kind(const MusubuValueHandle *value);
MUSUBU_API size_t MUSUBU_CALL value_get_length(const MusubuValueHandle *value);

/* Borrowed UTF-8 bytes without terminator, valid until value_free(value).
 * NULL/non-enum returns NULL and zero length. NULL length returns NULL.
 * length must not alias value storage. */
MUSUBU_API const char *MUSUBU_CALL value_get_enum_type(const MusubuValueHandle *value, size_t *length);
MUSUBU_API const char *MUSUBU_CALL value_get_enum_variant(const MusubuValueHandle *value, size_t *length);

/* NULL is a no-op. Do not use or free a handle again after this call. */
MUSUBU_API void MUSUBU_CALL value_free(MusubuValueHandle *value);

#ifdef __cplusplus
}
#endif

#endif
