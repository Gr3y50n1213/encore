# Root checks hand each track to the Makefile in that track's folder.
# Each track's Phase 1 creates that Makefile with a real `check` target (docs/ROADMAP.md).
#   make check-core      core/ and server/
#   make check-apple     apple/
#   make check-android   android/
#   make check-flutter   flutter/
#   make check           every track (CI runs this; a single computer usually cannot)
.PHONY: check check-core check-apple check-android check-flutter

check: check-core check-apple check-android check-flutter

check-core:
	@for d in core server; do \
	  if [ -f $$d/Makefile ]; then $(MAKE) -C $$d check || exit 1; \
	  else echo "$$d/Makefile does not exist yet: the core track's Phase 1 must create it (docs/ROADMAP.md)." >&2; exit 1; fi; \
	done

check-apple check-android check-flutter: check-%:
	@d=$*; if [ -f $$d/Makefile ]; then $(MAKE) -C $$d check; \
	else echo "$$d/Makefile does not exist yet: that track's Phase 1 must create it (docs/ROADMAP.md)." >&2; exit 1; fi
