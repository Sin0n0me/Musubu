#ifndef MUSUBU_H
#define MUSUBU_H

#include <stdbool.h>
#include <stddef.h>

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

#ifdef __cplusplus
}
#endif

#endif
