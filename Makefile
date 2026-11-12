# Dev helpers for working from a checkout. (End users install via install.sh.)
#
# Ways to run code you just wrote:
#   make dev      the desktop app on an isolated instance — own daemon, own data, own
#                 app settings; your real sessions untouched (first run copies your
#                 real projects, worktrees, workspaces and settings in, so it looks
#                 like yours — `make dev-reset` re-copies)
#
# Each checkout gets its own instance, keyed to its path, so the main clone and
# every worktree can run at once without sharing a daemon, a DB, or a port.
# `make dev-ls` shows them all.
#   make install  put it in ~/.cargo/bin for real use (then `make kill` to cut over)
#   make cycle    install + kill + dev in one go — the re-runnable full cutover

PREFIX      ?= $(HOME)/.cargo/bin
RELEASE_BIN := target/release/berth
DEBUG_BIN   := target/debug/berth

# The dev instance is a second, complete berth: its own socket, DB, and
# settings — and one *per checkout*, so the main clone and every worktree can
# run at the same time. Sharing them is worse than a port clash: two checkouts
# on one runtime dir means the second app silently attaches to the first's
# daemon and you drive the other checkout's binary, while `dev-prep` below
# SIGTERMs whichever daemon got there first.
#
# The slot is the checkout's directory name plus a hash of its absolute path,
# so two worktrees with the same name in different repos still separate. The
# runtime dir takes the hash alone — it holds a unix socket, and SUN_LEN (104
# bytes on macOS) is not a budget a long worktree name should be spending.
DEV_SLOT    := $(shell printf '%s' '$(CURDIR)' | shasum | cut -c1-8)
DEV_RUNTIME := /tmp/berth-dev-$(DEV_SLOT)
DEV_DATA    := $(HOME)/.berth-dev/$(notdir $(CURDIR))-$(DEV_SLOT)
# `make dev SEED=0` skips the first-run copy and starts the dev instance empty.
SEED ?= 1
# `make dev AGENT=/bin/cat` stubs agents out, so nothing spawns a real claude —
# including the warm-slot prewarm, which launches one before you create any
# agent at all. Unset (the default) means real agents, exactly like production.
AGENT ?=

# Every dev-instance run goes through this: its own socket dir and its own DB,
# so nothing here can touch the real daemon's state.
DEV_ENV = BERTH_RUNTIME_DIR=$(DEV_RUNTIME) BERTH_DATA_DIR=$(DEV_DATA) \
	$(if $(AGENT),BERTH_AGENT_CMD=$(AGENT))

.DEFAULT_GOAL := help
.PHONY: help dev dev-prep dev-seed dev-reset dev-ls dev-stop build install kill cycle check fmt lint test ci clean

help: ## Show this help
	@grep -hE '^[a-z][a-z-]*:.*?## ' $(MAKEFILE_LIST) \
		| awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-11s\033[0m %s\n", $$1, $$2}'

# --- running your changes ----------------------------------------------------

# The desktop app starts `berth daemon` from PATH, the installed build, when
# nothing answers, so the dev daemon has to be up first: `workspace list`
# auto-spawns it from this binary (`current_exe`). The app's own settings,
# connectors and secrets (a file, not the keychain) go to BERTH_HOME inside the
# dev data dir instead of your real ~/.berth.
dev: dev-prep ## Run the desktop app on the latest code in an isolated instance
	@echo "dev instance [$(notdir $(CURDIR))] → runtime $(DEV_RUNTIME), data $(DEV_DATA)"
	@$(DEV_ENV) $(DEBUG_BIN) workspace list >/dev/null
	-@$(DEV_ENV) BERTH_HOME=$(DEV_DATA)/home BERTH_SECRETS=file pnpm --filter @berth/desktop dev
	@$(MAKE) --no-print-directory dev-stop

# Build, clear the way, and seed — everything `dev` needs before it starts the
# app.
dev-prep:
	cargo build
	@# Load-bearing: a dev daemon from a previous run detached and outlived
	@# its app, and it is still executing the OLD code. Connecting to it is
	@# precisely how "I rebuilt and my change isn't there" happens — so stop
	@# it, and let this run spawn a fresh daemon from the binary above.
	@$(MAKE) --no-print-directory dev-stop
	@# Also load-bearing: on macOS the first exec of a freshly relinked binary
	@# pays for signature validation and can stall for seconds. Paying it here
	@# keeps the daemon spawn inside the client's 3s connect deadline
	@# (crates/berth/src/client.rs) instead of failing with "daemon did not come up".
	@$(DEBUG_BIN) --version >/dev/null
	@$(if $(filter 0,$(SEED)),true,$(MAKE) --no-print-directory dev-seed)

# A blank dev instance is useless for eyeballing a change — you'd re-add every
# project by hand first. So the first `make dev` snapshots the real DB and
# settings, minus `agents` and `terminals`: those rows are the live sessions
# the real daemon owns, and the dev daemon must not resume them. `.backup`
# reads the WAL, so the copy is consistent even with the real daemon running.
# The real dir is where `directories::ProjectDirs::from("dev","","berth")`
# puts it (berth-core/src/paths.rs), and pacer's (berth-core/src/legacy.rs)
# when berth has none yet. Keep the three in step with docs/configuration.md.
dev-seed: ## Copy real projects/workspaces/settings into the dev instance (only if it has no DB yet)
	@[ ! -e $(DEV_DATA)/berth.db ] || exit 0; \
	case "$$(uname -s)" in \
		Darwin) base="$$HOME/Library/Application Support/dev.";; \
		*)      base="$${XDG_DATA_HOME:-$$HOME/.local/share}/";; \
	esac; \
	real="$${base}berth"; db="$$real/berth.db"; \
	if [ ! -f "$$db" ]; then real="$${base}pacer"; db="$$real/pacer.db"; fi; \
	if [ ! -f "$$db" ]; then \
		echo "no real berth or pacer data under $$base - dev instance starts empty"; exit 0; fi; \
	if ! command -v sqlite3 >/dev/null 2>&1; then \
		echo "sqlite3 not on PATH - dev instance starts empty"; exit 0; fi; \
	mkdir -p $(DEV_DATA); \
	sqlite3 "$$db" ".backup '$(DEV_DATA)/berth.db'"; \
	sqlite3 $(DEV_DATA)/berth.db "DELETE FROM agents; DELETE FROM terminals;"; \
	for f in config.json; do \
		if [ -f "$$real/$$f" ]; then cp "$$real/$$f" $(DEV_DATA)/; fi; \
	done; \
	echo "seeded dev instance from $$real (projects, worktrees, workspaces, settings — no sessions)"

dev-reset: dev-stop ## Wipe this checkout's dev data; the next `make dev` re-seeds it
	rm -rf $(DEV_DATA)

# Slots accumulate: a worktree you deleted leaves its DB behind under
# ~/.berth-dev. This lists every one with its daemon's state, so you can see
# what is still running and `rm -rf` what is not.
dev-ls: ## List every checkout's dev instance and whether its daemon is up
	@for d in $(HOME)/.berth-dev/*-*/; do \
		[ -d "$$d" ] || continue; \
		slot=$${d%/}; slot=$${slot##*-}; \
		pidfile=/tmp/berth-dev-$$slot/daemon.pid; \
		state=stopped; \
		if [ -f "$$pidfile" ] && ps -p "$$(cat $$pidfile 2>/dev/null)" -o command= 2>/dev/null \
			| grep -q 'berth daemon'; then state=running; fi; \
		printf '  %-8s %-40s %s\n' "$$state" "$$(basename $$d)" "$$d"; \
	done

# The pidfile outlives the process it names, so confirm the pid is still a
# berth daemon before signalling it — otherwise a recycled pid means killing
# some unrelated process of the user's. SIGTERM (not KILL) so the daemon runs
# its normal shutdown and takes its PTY children with it.
dev-stop: ## Stop the dev daemon (it detaches, so quitting the app leaves it running)
	@pidfile=$(DEV_RUNTIME)/daemon.pid; \
	[ -f $$pidfile ] || exit 0; \
	pid=$$(cat $$pidfile 2>/dev/null); \
	case "$$pid" in ''|*[!0-9]*) exit 0;; esac; \
	if ps -p $$pid -o command= 2>/dev/null | grep -q 'berth daemon'; then \
		kill $$pid 2>/dev/null || true; \
	fi

# --- installing for real use -------------------------------------------------

build: ## Release build
	cargo build --release

# The cp+mv two-step is load-bearing on macOS: overwriting the installed
# binary in place reuses its inode, and the kernel's cached code signature
# for that inode no longer matches the new contents — every exec then dies
# with SIGKILL (exit 137). A fresh inode forces signature re-validation.
install: build ## Install to $(PREFIX) — warns if the live daemon is now stale
	cp $(RELEASE_BIN) $(PREFIX)/berth.new
	mv $(PREFIX)/berth.new $(PREFIX)/berth
	@$(PREFIX)/berth --version
	@$(PREFIX)/berth _stale-daemon-note

kill: ## Stop every session and the daemon — the cutover step after `make install`
	$(PREFIX)/berth kill

# The whole cutover as one command, safe to re-run as often as you like:
# install first, so a build that fails stops here with every session still
# alive; then kill the real daemon (it is now running the old binary — the
# STALE DAEMON NOTE `install` just printed says as much); then `dev`, which
# builds the debug binary and runs the desktop app on the isolated instance.
# `berth kill` exits 0 and says "no berth daemon running" when there is
# nothing to stop, so the first run on a cold machine goes through too. The
# kill stops every real session — run this from a terminal outside berth,
# not from a session it would take down with it. Recipe lines rather than
# prerequisites so `make -j` cannot reorder the three.
cycle: ## Install, kill the real daemon, run the dev instance — re-run whenever
	@$(MAKE) --no-print-directory install
	@$(MAKE) --no-print-directory kill
	@$(MAKE) --no-print-directory dev

# --- checks ------------------------------------------------------------------

check: ## Typecheck the workspace (fastest feedback)
	cargo check --workspace --all-targets

fmt: ## Format the workspace
	cargo fmt --all

# Not `-D warnings` by default, so a work-in-progress tree still lints. CI
# runs the strict form on both hosts, so `make lint STRICT=1` is what to
# check against before pushing.
lint: ## Clippy over the workspace (STRICT=1 to fail on warnings)
	cargo clippy --workspace --all-targets $(if $(STRICT),-- -D warnings)

test: ## Full test suite (e2e_pty spawns real daemons — slow)
	cargo test --workspace

ci: ## The whole gate: fmt check, clippy, tests
	cargo fmt --all -- --check
	@$(MAKE) --no-print-directory lint
	@$(MAKE) --no-print-directory test

clean: ## Remove build artifacts
	cargo clean
