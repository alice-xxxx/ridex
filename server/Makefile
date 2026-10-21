CC ?= cc
CFLAGS ?= -O2
CFLAGS += -I. -DMG_TLS=3 -DMG_ENABLE_CUSTOM_RANDOM=0 -Wall -Wextra -Werror

auth_server: main.c mongoose.c mongoose.h
	$(CC) $(CFLAGS) main.c mongoose.c -o $@

clean:
	rm -f auth_server
