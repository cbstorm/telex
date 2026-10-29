.PHONY: install install-user uninstall uninstall-user

# System-wide IBus engine install (uses sudo, restarts IBus).
install:
	./install.sh

# Per-user install, no sudo; log out and back in afterwards.
install-user:
	./install.sh --user

uninstall:
	./install.sh --uninstall

uninstall-user:
	./install.sh --uninstall --user
