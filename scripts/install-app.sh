#!/bin/bash
# macOS counterpart of install-app.ps1: builds the desktop app from a committed
# ref of a berth checkout into a working copy under the install root, keeps the
# build before it for rollback, and records which build the daemon runs.
#
#   bash scripts/install-app.sh [-Launch] [-Force] [-Repo DIR] [-Root DIR] [-Ref BRANCH|COMMIT]
#   bash scripts/install-app.sh -Rollback [-Launch]
#   bash scripts/install-app.sh -UpdateDaemon
#
# The flags match install-app.ps1 because the desktop app passes them; the
# `--kebab-case` spellings work too. Runs on the stock /bin/bash 3.2.

set -u

REPO=''
ROOT=''
REF='main'
FORCE=0
LAUNCH=0
ROLLBACK=0
UPDATE_DAEMON=0

while [ $# -gt 0 ]; do
    case "$1" in
        -Repo | --repo) REPO=$2; shift ;;
        -Root | --root) ROOT=$2; shift ;;
        -Ref | --ref) REF=$2; shift ;;
        -Force | --force) FORCE=1 ;;
        -Launch | --launch) LAUNCH=1 ;;
        -Rollback | --rollback) ROLLBACK=1 ;;
        -UpdateDaemon | --update-daemon) UPDATE_DAEMON=1 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
    shift
done

if [ "$(uname -s)" != Darwin ]; then
    echo 'install-app.sh supports macOS only' >&2
    exit 2
fi

[ -n "$ROOT" ] || ROOT=${BERTH_APP_ROOT:-"$HOME/Library/Application Support/Berth"}
APPLICATIONS=${BERTH_APPLICATIONS_DIR:-"$HOME/Applications"}
SIGN_IDENTITY=${BERTH_CODESIGN_IDENTITY:-}
if [ -z "$SIGN_IDENTITY" ] && [ -s "$ROOT/signing-identity" ]; then
    SIGN_IDENTITY=$(head -n 1 "$ROOT/signing-identity")
fi
[ -n "$SIGN_IDENTITY" ] || SIGN_IDENTITY=-
BUNDLE='Berth.app'

APP_DIR="$ROOT/app"
BUILD_DIR="$ROOT/build"
CURRENT="$APP_DIR/current"
PREVIOUS="$APP_DIR/previous"
STATUS_PATH="$BUILD_DIR/status.json"
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)

now() { date -u +%Y-%m-%dT%H:%M:%SZ; }

json_str() {
    local s=$1
    s=${s//\\/\\\\}
    s=${s//\"/\\\"}
    s=${s//$'\n'/\\n}
    s=${s//$'\r'/\\r}
    s=${s//$'\t'/\\t}
    printf '"%s"' "$s"
}

build_field() {
    local file="$1/build.json"
    [ -f "$file" ] || return 0
    sed -n "s/^ *\"$2\": *\"\\(.*\\)\",\\{0,1\\}$/\\1/p" "$file" | head -n 1
}

link_target() {
    [ -L "$1" ] && readlink "$1"
    return 0
}

start_berth() {
    open "$CURRENT/$BUNDLE"
}

link_applications() {
    local entry="$APPLICATIONS/$BUNDLE"
    mkdir -p "$APPLICATIONS"
    if [ -e "$entry" ] && [ ! -L "$entry" ]; then
        echo "left $entry alone: it is not a link this script made" >&2
        return 0
    fi
    ln -sfn "$CURRENT/$BUNDLE" "$entry"
}

if [ "$ROLLBACK" = 1 ]; then
    was=$(link_target "$CURRENT")
    then_dir=$(link_target "$PREVIOUS")
    if [ -z "$then_dir" ] || [ ! -d "$then_dir/$BUNDLE" ]; then
        echo "no previous build under $APP_DIR" >&2
        exit 1
    fi
    ln -sfn "$then_dir" "$CURRENT"
    [ -n "$was" ] && ln -sfn "$was" "$PREVIOUS"
    echo "current build: $(build_field "$CURRENT" buildId) $(build_field "$CURRENT" subject)"
    [ "$LAUNCH" = 1 ] && start_berth
    exit 0
fi

if [ "$UPDATE_DAEMON" = 1 ]; then
    built="$CURRENT/berth"
    if [ ! -f "$built" ]; then
        echo "no berth in $CURRENT" >&2
        exit 1
    fi
    prefix=${BERTH_BIN_DIR:-"$HOME/.cargo/bin"}
    mkdir -p "$prefix"
    installed="$prefix/berth"
    cp "$built" "$installed.new" || exit 1
    chmod 755 "$installed.new"
    mv -f "$installed.new" "$installed" || exit 1
    "$installed" --version
    "$installed" kill
    nohup "$installed" daemon </dev/null >/dev/null 2>&1 &
    mkdir -p "$ROOT/daemon"
    cp -f "$CURRENT/build.json" "$ROOT/daemon/build.json"
    echo "daemon restarted from $installed"
    exit 0
fi

if [ -z "$REPO" ]; then
    if [ -e "$SCRIPT_DIR/../.git" ]; then
        REPO="$SCRIPT_DIR/.."
    else
        REPO=$(build_field "$SCRIPT_DIR" sourceRepo)
    fi
    if [ -z "$REPO" ]; then
        echo 'pass -Repo <path to the berth checkout>' >&2
        exit 2
    fi
fi
REPO=$(cd "$REPO" && pwd) || exit 2

mkdir -p "$BUILD_DIR/logs" "$APP_DIR"
LOG="$BUILD_DIR/logs/$(date +%Y%m%d-%H%M%S).log"
: >"$LOG"

STATE=running
STEP=start
STARTED=$(now)
STEPS_JSON="\"start\": $(json_str "$STARTED")"
COMMIT=''
SUBJECT=''
BUILD_ID=''
PROTOCOL=''
FINISHED=''
ERROR=''
UP_TO_DATE=''

save_status() {
    {
        printf '{\n'
        printf '  "state": %s,\n' "$(json_str "$STATE")"
        printf '  "step": %s,\n' "$(json_str "$STEP")"
        printf '  "pid": %s,\n' "$$"
        printf '  "repo": %s,\n' "$(json_str "$REPO")"
        printf '  "startedAt": %s,\n' "$(json_str "$STARTED")"
        printf '  "steps": {%s},\n' "$STEPS_JSON"
        [ -n "$COMMIT" ] && printf '  "commit": %s,\n' "$(json_str "$COMMIT")"
        [ -n "$SUBJECT" ] && printf '  "subject": %s,\n' "$(json_str "$SUBJECT")"
        [ -n "$UP_TO_DATE" ] && printf '  "upToDate": true,\n'
        [ -n "$BUILD_ID" ] && printf '  "buildId": %s,\n' "$(json_str "$BUILD_ID")"
        [ -n "$PROTOCOL" ] && printf '  "protocol": %s,\n' "$PROTOCOL"
        [ -n "$FINISHED" ] && printf '  "finishedAt": %s,\n' "$(json_str "$FINISHED")"
        [ -n "$ERROR" ] && printf '  "error": %s,\n' "$(json_str "$ERROR")"
        printf '  "log": %s\n' "$(json_str "$LOG")"
        printf '}\n'
    } >"$STATUS_PATH.tmp" && mv -f "$STATUS_PATH.tmp" "$STATUS_PATH"
}

add_log() { printf '%s\n' "$1" >>"$LOG"; }

fail() {
    STATE=failed
    ERROR=$1
    FINISHED=$(now)
    save_status
    add_log "failed: $1"
    echo "build failed: $1 (log: $LOG)" >&2
    exit 1
}

set_step() {
    STEP=$1
    STEPS_JSON="$STEPS_JSON, $(json_str "$1"): $(json_str "$(now)")"
    save_status
    add_log "== $1"
}

run() {
    add_log "> $*"
    "$@" >>"$LOG" 2>&1 || fail "$* exited with code $?"
}

save_status

COMMIT=$(git -C "$REPO" rev-parse --verify "$REF^{commit}" 2>/dev/null) || fail "no commit $REF in $REPO"
SUBJECT=$(git -C "$REPO" log -1 --format=%s "$COMMIT")
save_status

if [ "$FORCE" = 0 ] && [ "$(build_field "$CURRENT" commit)" = "$COMMIT" ]; then
    STATE=done
    UP_TO_DATE=1
    BUILD_ID=$(build_field "$CURRENT" buildId)
    FINISHED=$(now)
    save_status
    echo "already up to date: $BUILD_ID"
    [ "$LAUNCH" = 1 ] && start_berth
    exit 0
fi

set_step export
SRC="$BUILD_DIR/src"
mkdir -p "$SRC"
GIT_DIR_ABS=$(git -C "$REPO" rev-parse --absolute-git-dir) || fail "no git dir in $REPO"
add_log "> git read-tree --reset -u $COMMIT"
GIT_INDEX_FILE="$BUILD_DIR/src.index" git --git-dir="$GIT_DIR_ABS" --work-tree="$SRC" read-tree --reset -u "$COMMIT" >>"$LOG" 2>&1 \
    || fail "git read-tree $COMMIT failed"

export CARGO_TARGET_DIR="$BUILD_DIR/target"
export CARGO_PROFILE_RELEASE_LTO=false
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
export CARGO_PROFILE_RELEASE_INCREMENTAL=true
export CARGO_PROFILE_RELEASE_OPT_LEVEL=2

DAEMON_EXE="$CARGO_TARGET_DIR/release/berth"
INSTALLED_COMMIT=$(build_field "$CURRENT" commit)
if [ -n "$INSTALLED_COMMIT" ] && [ -x "$CURRENT/berth" ] \
    && git -C "$REPO" diff --quiet "$INSTALLED_COMMIT" "$COMMIT" -- crates vendor Cargo.toml Cargo.lock rust-toolchain.toml .cargo 2>/dev/null; then
    DAEMON_EXE="$CURRENT/berth"
fi
cd "$SRC" || fail "cannot enter $SRC"
set_step deps
run pnpm install --frozen-lockfile --config.confirmModulesPurge=false
set_step tokens
run pnpm build:tokens
set_step app
run pnpm --filter @berth/desktop exec tauri build --bundles app
if [ "$DAEMON_EXE" = "$CURRENT/berth" ]; then
    add_log "daemon sources unchanged since $INSTALLED_COMMIT, reusing $DAEMON_EXE"
else
    set_step daemon
    run cargo build --release --locked -p berth
fi

set_step install
RELEASE="$CARGO_TARGET_DIR/release"
PROTOCOL=$(sed -n 's/.*pub const PROTOCOL_VERSION: u32 = \([0-9][0-9]*\);.*/\1/p' "$SRC/crates/berth-core/src/protocol.rs" | head -n 1)
[ -n "$PROTOCOL" ] || fail 'no PROTOCOL_VERSION in protocol.rs'
BUILD_ID="$(date +%Y%m%d-%H%M%S)-$(printf '%s' "$COMMIT" | cut -c1-8)"
TARGET="$APP_DIR/$BUILD_ID"
STAGING="$TARGET.partial"
rm -rf "$STAGING"
mkdir -p "$STAGING"
run ditto "$RELEASE/bundle/macos/$BUNDLE" "$STAGING/$BUNDLE"
run codesign --force --deep --sign "$SIGN_IDENTITY" "$STAGING/$BUNDLE"
run cp "$DAEMON_EXE" "$STAGING/berth"
run cp "$SRC/scripts/install-app.sh" "$STAGING/install-app.sh"
{
    printf '{\n'
    printf '  "buildId": %s,\n' "$(json_str "$BUILD_ID")"
    printf '  "commit": %s,\n' "$(json_str "$COMMIT")"
    printf '  "subject": %s,\n' "$(json_str "$SUBJECT")"
    printf '  "sourceRepo": %s,\n' "$(json_str "$REPO")"
    printf '  "builtAt": %s,\n' "$(json_str "$(now)")"
    printf '  "protocol": %s\n' "$PROTOCOL"
    printf '}\n'
} >"$STAGING/build.json"
mv "$STAGING" "$TARGET" || fail "cannot move $STAGING into place"

WAS=$(link_target "$CURRENT")
ln -sfn "$TARGET" "$CURRENT"
[ -n "$WAS" ] && ln -sfn "$WAS" "$PREVIOUS"
link_applications

RUNNING=$(ps -axo command=)
for dir in "$APP_DIR"/*; do
    [ -d "$dir" ] && [ ! -L "$dir" ] || continue
    [ "$dir" = "$TARGET" ] && continue
    [ -n "$WAS" ] && [ "$dir" = "$WAS" ] && continue
    case "$RUNNING" in *"$dir/"*) continue ;; esac
    rm -rf "$dir"
done

STATE=done
STEP=install
FINISHED=$(now)
save_status
add_log "installed $BUILD_ID into $TARGET"
echo "installed $BUILD_ID ($SUBJECT) into $TARGET"
[ "$LAUNCH" = 1 ] && start_berth
exit 0
