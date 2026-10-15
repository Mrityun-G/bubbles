# Builds the C host example against the bubbles-ffi shared library.
#
#   make        - build the shared library (via cargo) and then this example
#   make run    - build, then run it on tavern.bub
#   make clean  - remove the example binary
#
# LIBDIR defaults to the workspace's release output. Point it at an extracted
# prebuilt-binaries archive to build without a Rust toolchain:
#
#   make LIBDIR=/path/to/bubbles-ffi-x86_64-unknown-linux-gnu NO_CARGO=1

ROOT    := $(abspath ../..)
LIBDIR  ?= $(ROOT)/target/release
INCDIR  ?= $(ROOT)/crates/bubbles-ffi/include
BIN     := c-host

CFLAGS  += -std=c99 -Wall -Wextra -O2 -I$(INCDIR)
LDFLAGS += -L$(LIBDIR) -lbubbles_ffi

# Embed the library path so the binary runs without LD_LIBRARY_PATH / DYLD_LIBRARY_PATH.
ifeq ($(shell uname -s),Darwin)
  LDFLAGS += -Wl,-rpath,$(LIBDIR)
else
  LDFLAGS += -Wl,-rpath,$(LIBDIR) -Wl,--enable-new-dtags
endif

.PHONY: all run clean lib

all: $(BIN)

# Skipped when NO_CARGO is set, so the example can be built against a prebuilt binary.
lib:
ifndef NO_CARGO
	cargo build --manifest-path $(ROOT)/Cargo.toml -p bubbles-ffi --release
endif

$(BIN): main.c lib
	$(CC) $(CFLAGS) main.c $(LDFLAGS) -o $@

run: $(BIN)
	./$(BIN) tavern.bub Tavern

clean:
	rm -f $(BIN)
