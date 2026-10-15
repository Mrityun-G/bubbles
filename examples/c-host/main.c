/*
 * A complete, dependency-free C host for bubbles dialogue.
 *
 * Loads a .bub file from disk, pumps the event loop, prints lines, and selects an option.
 * Interactive when stdin is a terminal; otherwise it picks the first available option so
 * the same binary works unattended in CI.
 *
 * Build and run: see README.md in this directory, or just `make run`.
 *
 * Everything here uses only the C ABI in bubbles_ffi.h - there is no Rust in this file and
 * no Rust toolchain needed to run it once you have the shared library.
 *
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "bubbles_ffi.h"

#ifdef _WIN32
#include <io.h>
#define stdin_is_tty() _isatty(_fileno(stdin))
#else
#include <unistd.h>
#define stdin_is_tty() isatty(STDIN_FILENO)
#endif

/* ── Error reporting ──────────────────────────────────────────────────────────── */

/* bubbles_last_error() may return NULL, and its pointer dies on the next bubbles_* call,
 * so read it immediately and never store it. */
static void fail(const char *what) {
    const char *msg = bubbles_last_error();
    fprintf(stderr, "error: %s: %s\n", what, msg ? msg : "(no message)");
}

/* ── Tiny JSON reader ─────────────────────────────────────────────────────────
 *
 * Events arrive as JSON. A real game would use its engine's JSON parser; this is just
 * enough to read the fields the example needs, without a dependency. It understands
 * string and boolean fields and can walk an array element-by-element, which is all the
 * event schema requires.
 *
 * Scanning respects string literals and nesting, so it does not confuse a `"text"` key
 * inside a nested object with one at the level it is looking at.
 */

/* Advances past a JSON string literal; `p` must point at the opening quote. */
static const char *skip_string(const char *p) {
    for (++p; *p; ++p) {
        if (*p == '\\' && p[1]) {
            ++p;
        } else if (*p == '"') {
            return p + 1;
        }
    }
    return p;
}

/* Finds `"key"` at the top level of the object at `json`, and returns the first character
 * of its value. Returns NULL if the key is absent at this level.
 *
 * Only matches a string that is followed by ':', so a string *value* that happens to equal
 * the key - a speaker literally named "text" - is not mistaken for the key. */
static const char *find_field(const char *json, size_t len, const char *key) {
    const char *end = json + len;
    size_t key_len = strlen(key);
    int depth = 0;

    for (const char *p = json; p < end && *p; ) {
        if (*p == '"') {
            const char *close = skip_string(p);
            const char *after = close;
            while (after < end && *after == ' ') ++after;

            int is_key = after < end && *after == ':';
            if (is_key && depth == 1 && (size_t)(close - p) == key_len + 2 &&
                strncmp(p + 1, key, key_len) == 0) {
                const char *v = after + 1;
                while (v < end && *v == ' ') ++v;
                return v < end ? v : NULL;
            }
            p = close;
            continue;
        }
        if (*p == '{' || *p == '[') ++depth;
        if (*p == '}' || *p == ']') --depth;
        ++p;
    }
    return NULL;
}

/* Copies a string field into `out`, decoding the escapes the runtime can emit. Returns 0
 * if the field is missing or is JSON null (an absent speaker, for instance). */
static int string_field(const char *json, size_t len, const char *key, char *out,
                        size_t out_cap) {
    const char *v = find_field(json, len, key);
    if (!v || *v != '"') return 0;

    size_t n = 0;
    for (const char *p = v + 1; *p && *p != '"' && n + 1 < out_cap; ++p) {
        if (*p == '\\' && p[1]) {
            switch (*++p) {
                case 'n':  out[n++] = '\n'; break;
                case 't':  out[n++] = '\t'; break;
                case 'r':  out[n++] = '\r'; break;
                case 'u':  out[n++] = '?'; p += 4; break; /* good enough for a demo */
                default:   out[n++] = *p;   break;        /* covers \" \\ \/ */
            }
        } else {
            out[n++] = *p;
        }
    }
    out[n] = '\0';
    return 1;
}

static int bool_field(const char *json, size_t len, const char *key) {
    const char *v = find_field(json, len, key);
    return v && strncmp(v, "true", 4) == 0;
}

/* Locates element `index` of the JSON array starting at `arr` (which points at '['), and
 * writes its extent to `*out_len`. Returns NULL once the array is exhausted. */
static const char *array_element(const char *arr, size_t index, size_t *out_len) {
    if (*arr != '[') return NULL;

    const char *p = arr + 1;
    for (size_t i = 0; ; ++i) {
        while (*p == ' ' || *p == ',' || *p == '\n') ++p;
        if (*p == ']' || !*p) return NULL;

        /* Walk to the end of this element: a balanced object or array, a string, or a
         * bare scalar terminated by ',' or the array's own ']'. */
        const char *start = p;
        int depth = 0;
        while (*p) {
            if (*p == '"') {
                p = skip_string(p);
                if (depth == 0) break;
                continue;
            }
            if (*p == '{' || *p == '[') { ++depth; ++p; continue; }
            if (*p == '}' || *p == ']') {
                if (depth == 0) break;              /* the enclosing array ended */
                if (--depth == 0) { ++p; break; }   /* this element ended */
                ++p;
                continue;
            }
            if (*p == ',' && depth == 0) break;     /* end of a bare scalar */
            ++p;
        }

        if (i == index) {
            *out_len = (size_t)(p - start);
            return start;
        }
    }
}

/* ── File loading ─────────────────────────────────────────────────────────────── */

static char *read_file(const char *path, size_t *out_len) {
    FILE *f = fopen(path, "rb");
    if (!f) {
        fprintf(stderr, "error: cannot open %s\n", path);
        return NULL;
    }
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    rewind(f);
    if (size < 0) {
        fclose(f);
        fprintf(stderr, "error: cannot measure %s\n", path);
        return NULL;
    }

    char *buf = malloc((size_t)size + 1);
    if (!buf) {
        fclose(f);
        fprintf(stderr, "error: out of memory reading %s\n", path);
        return NULL;
    }
    *out_len = fread(buf, 1, (size_t)size, f);
    buf[*out_len] = '\0';
    fclose(f);
    return buf;
}

/* ── Event handlers ───────────────────────────────────────────────────────────── */

#define FIELD_MAX 1024

static void show_line(const char *json, size_t len) {
    char speaker[FIELD_MAX];
    char text[FIELD_MAX];

    if (!string_field(json, len, "text", text, sizeof text)) return;

    /* `speaker` is JSON null for narration, so string_field returns 0. */
    if (string_field(json, len, "speaker", speaker, sizeof speaker)) {
        printf("%s: %s\n", speaker, text);
    } else {
        printf("%s\n", text);
    }
}

static void show_command(const char *json, size_t len) {
    char name[FIELD_MAX];
    if (string_field(json, len, "name", name, sizeof name)) {
        printf("  [command: %s]\n", name);
    }
}

/* Prints the options, chooses one, and tells the runner. Returns 0 on success. */
static int choose_option(void *runner, const char *json, size_t len) {
    const char *options = find_field(json, len, "options");
    if (!options || *options != '[') {
        fprintf(stderr, "error: Options event had no options array\n");
        return 1;
    }

    size_t count = 0;
    long first_available = -1;
    for (;; ++count) {
        size_t opt_len;
        const char *opt = array_element(options, count, &opt_len);
        if (!opt) break;

        char text[FIELD_MAX];
        if (!string_field(opt, opt_len, "text", text, sizeof text)) continue;

        /* An unavailable option failed its `<<if>>` guard. Selecting one returns
         * BUBBLES_ERR, so a real UI greys it out or hides it. */
        int available = bool_field(opt, opt_len, "available");
        printf("  %zu) %s%s\n", count + 1, text, available ? "" : "  (unavailable)");
        if (available && first_available < 0) first_available = (long)count;
    }

    if (first_available < 0) {
        fprintf(stderr, "error: no option is available; the script cannot continue\n");
        return 1;
    }

    long picked = first_available;
    if (stdin_is_tty()) {
        printf("Choose [1-%zu]: ", count);
        fflush(stdout);

        char buf[32];
        if (fgets(buf, sizeof buf, stdin)) {
            long n = strtol(buf, NULL, 10);
            if (n >= 1 && (size_t)n <= count) picked = n - 1;
        }
    }
    printf("> %ld\n", picked + 1);

    /* The index is a size_t: pointer-width, not int. */
    if (bubbles_runner_select_option(runner, (size_t)picked) != BUBBLES_OK) {
        fail("bubbles_runner_select_option");
        return 1;
    }
    return 0;
}

/* ── Main ─────────────────────────────────────────────────────────────────────── */

int main(int argc, char **argv) {
    const char *path = argc > 1 ? argv[1] : "tavern.bub";
    const char *node = argc > 2 ? argv[2] : "Tavern";

    /* Step 1: the version handshake. Refuse to run against an ABI we do not understand
     * rather than discovering the mismatch as memory corruption later. */
    uint32_t abi = bubbles_abi_version();
    if (abi != BUBBLES_ABI_VERSION) {
        fprintf(stderr, "error: bubbles ABI mismatch: header %d, library %u\n",
                BUBBLES_ABI_VERSION, abi);
        return 1;
    }

    /* Step 2: load the .bub source. The library takes UTF-8 bytes plus a byte length and
     * copies what it needs, so this buffer only has to outlive the compile call. */
    size_t source_len = 0;
    char *source = read_file(path, &source_len);
    if (!source) return 1;

    int status = 1;
    int rc = 0;
    int32_t exists = 0;
    void *program = NULL;
    void *runner = NULL;

    /* Step 3: compile. */
    if (bubbles_compile(source, source_len, &program) != BUBBLES_OK) {
        fail("bubbles_compile");
        goto cleanup;
    }

    /* Optional, but it turns a typo into a clear message instead of a start failure.
     * Program introspection only works before the program is handed to a runner. */
    if (bubbles_program_node_exists(program, node, strlen(node), &exists) != BUBBLES_OK) {
        fail("bubbles_program_node_exists");
        goto cleanup;
    }
    if (!exists) {
        fprintf(stderr, "error: %s has no node titled \"%s\"\n", path, node);
        goto cleanup;
    }

    /* Step 4: build a runner. This CONSUMES the program handle - on success and on
     * failure - so drop our copy immediately. Freeing it later would be a double free. */
    rc = bubbles_runner_new(program, &runner);
    program = NULL;
    if (rc != BUBBLES_OK) {
        fail("bubbles_runner_new");
        goto cleanup;
    }

    /* Step 5: start at a node. */
    if (bubbles_runner_start(runner, node, strlen(node)) != BUBBLES_OK) {
        fail("bubbles_runner_start");
        goto cleanup;
    }

    /* Step 6: pump events until BUBBLES_DONE. */
    for (;;) {
        char *event = NULL;
        int ev = bubbles_runner_next_event(runner, &event);

        if (ev == BUBBLES_DONE) break;
        if (ev != BUBBLES_OK) {
            fail("bubbles_runner_next_event");
            goto cleanup;
        }

        size_t len = strlen(event);
        char kind[64];
        int err = 0;

        if (string_field(event, len, "kind", kind, sizeof kind)) {
            if (strcmp(kind, "Line") == 0) {
                show_line(event, len);
            } else if (strcmp(kind, "Command") == 0) {
                show_command(event, len);
            } else if (strcmp(kind, "Options") == 0) {
                err = choose_option(runner, event, len);
            }
            /* NodeStarted, NodeComplete, DialogueComplete, and any kind a future runtime
             * adds (including "Unknown") need no handling here - ignore and keep pumping.
             * Treating an unrecognised kind as fatal would break on a library upgrade. */
        }

        /* Every string the library returns is freed with bubbles_string_free, never
         * free(). The library may use a different allocator than this program. */
        bubbles_string_free(event);

        if (err != 0) goto cleanup;
    }

    status = 0;

cleanup:
    /* All three frees accept NULL, so this path is safe however we arrived here. */
    bubbles_program_free(program);
    bubbles_runner_free(runner);
    free(source);
    return status;
}
