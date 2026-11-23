BIN := gdrv
CARGO ?= cargo

.PHONY: build release test lint fmt fmt-check clean install deps smoke verify dev

# Every target that produces a binary signs it (family-cli): an ad-hoc
# signature is a new code identity on every build, and the keychain's
# "Always Allow" does not carry over to one.
build: SIGN_TARGET = target/debug/$(BIN)
build:
	$(CARGO) build --locked
	@$(SIGN)

release: SIGN_TARGET = target/release/$(BIN)
release:
	$(CARGO) build --release --locked
	@$(SIGN)

test:
	$(CARGO) test --locked

lint:
	$(CARGO) clippy --all-targets --locked -- -D warnings

fmt:
	$(CARGO) fmt

fmt-check:
	$(CARGO) fmt --check

clean:
	$(CARGO) clean

deps:
	$(CARGO) fetch --locked

# `--locked` builds from Cargo.lock, the versions the tests and audit ran on.
install: SIGN_TARGET = $${CARGO_INSTALL_ROOT:-$$HOME/.cargo}/bin/$(BIN)
install:
	$(CARGO) install --path . --locked --force
	@$(SIGN)

dev: SIGN_TARGET = target/debug/$(BIN)
dev:
	$(CARGO) build --locked
	@$(SIGN)

# Offline: no Drive, no keychain. The binary runs with a home under target/
# (`make clean` removes it), an empty keychain service and, in this debug
# build, the vault's key in a file, so a freshly built binary never waits on
# a keychain dialog here.
SMOKE_HOME := $(CURDIR)/target/smoke-home
SMOKE_ENV := HOME="$(SMOKE_HOME)" XDG_CONFIG_HOME="$(SMOKE_HOME)/config" \
	XDG_CACHE_HOME="$(SMOKE_HOME)/cache" GDRV_KEYCHAIN_SERVICE=piekstra.gdrv.hermetic-tests \
	GDRV_TEST_KEY_FILE="$(SMOKE_HOME)/keys.json"
SMOKE_HELP := login status logout about client profiles folder auth ls cat cp mv rm mkdir md5sum \
	duplicates config self-update completions info
smoke: build
	@set -e; mkdir -p "$(SMOKE_HOME)"; \
	run() { env $(SMOKE_ENV) ./target/debug/$(BIN) "$$@"; }; \
	run --version >/dev/null; \
	run info --json >/dev/null; \
	run completions bash >/dev/null; \
	run profiles list --json >/dev/null; \
	run config show --json >/dev/null; \
	for c in $(SMOKE_HELP); do run $$c --help >/dev/null; done; \
	code=0; run --json status </dev/null >/dev/null 2>&1 || code=$$?; \
	[ "$$code" = 2 ] || { echo "status with no profile exited $$code, not 2"; exit 1; }; \
	code=0; run --json rm gdrive:smoke/root/x.txt </dev/null >/dev/null 2>&1 || code=$$?; \
	[ "$$code" = 6 ] || { echo "rm without --force exited $$code, not 6"; exit 1; }; \
	echo "smoke ok"

verify: fmt-check lint test smoke

define SIGN
if [ -x "$$HOME/Dev/cli-common/scripts/dev-sign.sh" ]; then \
	"$$HOME/Dev/cli-common/scripts/dev-sign.sh" "$(SIGN_TARGET)"; \
else echo "cli-common/scripts/dev-sign.sh not found — $(SIGN_TARGET) left ad-hoc signed"; fi
endef
