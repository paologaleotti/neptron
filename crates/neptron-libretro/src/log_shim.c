/*
 * The log function that the host gives to cores through
 * RETRO_ENVIRONMENT_GET_LOG_INTERFACE.
 *
 * The libretro log callback is variadic, like printf, and stable Rust
 * cannot define a variadic function. This file formats the message and
 * passes the text to one Rust function, the sink, which the host sets
 * once before it loads a core.
 */

#include <stdarg.h>
#include <stdatomic.h>
#include <stddef.h>
#include <stdio.h>

/* Longer messages are cut. The Rust side keeps the same limit. */
#define NEPTRON_LOG_TEXT_BYTES 1024

typedef void (*neptron_libretro_log_sink)(unsigned level, const char *text);

/* Set from the host thread, read from any thread a core logs from. */
static _Atomic(neptron_libretro_log_sink) sink = NULL;

void neptron_libretro_set_log_sink(neptron_libretro_log_sink new_sink)
{
   atomic_store(&sink, new_sink);
}

void neptron_libretro_log_printf(unsigned level, const char *fmt, ...)
{
   char text[NEPTRON_LOG_TEXT_BYTES];
   neptron_libretro_log_sink current = atomic_load(&sink);
   va_list args;
   int written;

   if (fmt == NULL || current == NULL)
      return;

   va_start(args, fmt);
   written = vsnprintf(text, sizeof text, fmt, args);
   va_end(args);

   if (written < 0)
      return;

   current(level, text);
}
