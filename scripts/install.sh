#!/bin/sh
# Install, upgrade, roll back, trim and remove a Context Foundry package. The
# contract (layout, refusals, recovery, what removal keeps) is
# docs/deployment.md § Lifecycle and installation. Layout under a prefix P:
#
#   P/bin/foundry -> ../lib/context-foundry/current/bin/foundry
#   P/lib/context-foundry/current -> <version>    (switched atomically)
#   P/lib/context-foundry/<version>/               one per installed version
#   P/lib/context-foundry/installed.json   owned files + hashes, created dirs
#   P/lib/context-foundry/pending.json     the intent of a command in progress
#   P/lib/context-foundry/.lock/           one lifecycle command at a time
#
# A package is unpacked and verified against its PACKAGE.json, and its
# worker bundles and installed profile copies are built, in a private
# temporary directory. Before its first change, every mutating command
# writes pending.json: operation, from/to versions, the target's inventory
# (path -> SHA-256), every directory and temporary it creates, the intended
# disabled set, the hash of the installed.json it will commit, and (for
# uninstall) whether its host blocks are removed. Under the lock and only
# when no installed executable runs, the next command finalizes a record
# whose installed.json was committed, else completes an interrupted switch
# (the links select the target and every inventory file has its recorded
# hash) or rolls it back (links restored; only listed paths whose content
# still matches, or listed directories now empty, removed); disable and
# uninstall are completed. It refuses unless `current` is absent where
# allowed or names a recorded endpoint. Ownership is never inferred from a
# live directory or a name.
#
# Tests only: CF_TEST_HOOK=kill:POINT (after-place, after-current,
# after-links, after-commit; uninstall's after-pending, after-hosts) or
# wait:before-cutover:DIR, honoured only when the version being switched to
# (for disable and uninstall, the current one) is labelled `*-test.*`
# (package.sh gives that label only to prebuilt test binaries).
#
# Exit codes: 64 usage or argument, 65 verification or layout, 66 missing
# input, 69 component not in the package, 70 internal, 73 exists, 75 busy.
set -eu
LC_ALL=C
export LC_ALL

usage() {
    cat >&2 <<USAGE
usage: $0 install   --package TGZ --prefix P [COMPONENTS]
       $0 upgrade   --package TGZ --prefix P [COMPONENTS] [--store DIR]...
       $0 rollback  --prefix P
       $0 disable-semantic --prefix P | disable-learning --prefix P
       $0 uninstall --prefix P [--host-config FILE]...
COMPONENTS: [--semantic-profile FILE [--semantic-extra-read DIR]...] [--learning-profile FILE]
USAGE
    exit 64
}

die() {
    code=$1
    shift
    echo "install.sh: $*" >&2
    exit "$code"
}

sha() {
    shasum -a 256 "$1" | cut -d' ' -f1
}

# Every option takes one path. A value canonicalization, splitting or lsof's
# name escaping could rewrite is refused, never reinterpreted.
check_value() {
    case "$2" in
        "" | */) die 64 "refusing $1 '$2': empty or ending in /" ;;
    esac
    rest=$(printf '%s' "$2" | tr -d ' -~'; printf x)
    [ "$rest" = x ] || die 64 "refusing $1: it holds a byte outside printable ASCII (0x20-0x7E)"
}

COMMAND=${1:-}
[ $# -eq 0 ] || shift
PACKAGE=""
PREFIX=""
SEMANTIC_PROFILE=""
LEARNING_PROFILE=""
EXTRA_READS=0
STORES=0
HOST_CONFIGS=0
# The arguments after the command are OPTION VALUE pairs. They stay in "$@"
# so a repeatable option's values are each one argument (`for_each`).
parse() {
    while [ $# -gt 0 ]; do
        [ $# -ge 2 ] || usage
        check_value "$1" "$2"
        case "$1" in
            --package) PACKAGE=$2 ;;
            --prefix) PREFIX=$2 ;;
            --semantic-profile) SEMANTIC_PROFILE=$2 ;;
            --learning-profile) LEARNING_PROFILE=$2 ;;
            --semantic-extra-read)
                [ -d "$2" ] || die 66 "no directory $2"
                EXTRA_READS=$((EXTRA_READS + 1))
                ;;
            --store) STORES=$((STORES + 1)) ;;
            --host-config) HOST_CONFIGS=$((HOST_CONFIGS + 1)) ;;
            *) usage ;;
        esac
        shift 2
    done
}
parse "$@"

# for_each OPTION FUNCTION "$@": FUNCTION VALUE for each value of OPTION.
for_each() {
    want=$1
    fn=$2
    shift 2
    while [ $# -gt 0 ]; do
        [ "$1" != "$want" ] || "$fn" "$2"
        shift 2
    done
}

[ -n "$PREFIX" ] || usage
case "$COMMAND" in
    install | upgrade) [ -n "$PACKAGE" ] || usage ;;
    rollback | disable-semantic | disable-learning | uninstall) [ -z "$PACKAGE" ] || usage ;;
    *) usage ;;
esac
case "$COMMAND" in
    install | upgrade) ;;
    *) [ -z "$SEMANTIC_PROFILE$LEARNING_PROFILE" ] && [ "$EXTRA_READS" = 0 ] || usage ;;
esac
[ "$COMMAND" = upgrade ] || [ "$STORES" = 0 ] || usage
[ "$COMMAND" = uninstall ] || [ "$HOST_CONFIGS" = 0 ] || usage
[ "$EXTRA_READS" = 0 ] || [ -n "$SEMANTIC_PROFILE" ] || die 64 "--semantic-extra-read needs --semantic-profile"
for profile in "$SEMANTIC_PROFILE" "$LEARNING_PROFILE"; do
    [ -z "$profile" ] || [ -f "$profile" ] || die 66 "profile is not a file: $profile"
done
[ -z "$PACKAGE" ] || [ -f "$PACKAGE" ] || die 66 "no package at $PACKAGE"

if [ "$COMMAND" = install ]; then
    mkdir -p "$PREFIX"
fi
[ -d "$PREFIX" ] || die 66 "no prefix directory $PREFIX"
PREFIX=$(cd "$PREFIX" && pwd -P)
check_value --prefix "$PREFIX"
LIB=$PREFIX/lib/context-foundry
STATE=$LIB/installed.json
PENDING=$LIB/pending.json
LOCK=$LIB/.lock
BIN_LINK_TARGET=../lib/context-foundry/current/bin/foundry

TMP=$(mktemp -d "${TMPDIR:-/tmp}/cf-install.XXXXXX")
LOCKED=""
cleanup() {
    if [ -n "$LOCKED" ]; then
        rm -f "$LOCK/owner"
        rmdir "$LOCK" 2>/dev/null || true
    fi
    rm -rf "$TMP"
}
trap cleanup EXIT
: > "$TMP/empty"

# --- records -------------------------------------------------------------

json_value() {
    plutil -extract "$2" raw -o - -- "$1" 2>/dev/null || die 65 "$1 has no $2"
}

# "VALUE  KEY" lines of one JSON object. plutil prints it as an XML property
# list; recorded keys and values never need XML escaping.
sums_of() {
    plutil -extract "$2" xml1 -o "$TMP/sums.plist" -- "$1" 2>/dev/null || die 65 "$1 has no $2"
    awk '
        /^\t<key>/ { sub(/^\t<key>/, ""); sub(/<\/key>$/, ""); key = $0; next }
        /^\t<string>/ { sub(/^\t<string>/, ""); sub(/<\/string>$/, ""); print $0 "  " key }
    ' "$TMP/sums.plist"
}

# The keys of one JSON object, one per line.
keys_of() {
    plutil -extract "$2" raw -o - -- "$1" > "$TMP/keys" 2>/dev/null || die 65 "$1 has no $2"
    sed '/^$/d' "$TMP/keys"
}

# A JSON object body from "VALUE  KEY" lines (plain keys and values).
pairs_body() {
    sort -u -k2 "$1" | awk '{ printf "%s  \"%s\": \"%s\"", (NR > 1 ? ",\n" : ""), $2, $1 }'
}

# A JSON object body whose keys are the lines of $1 (escaped), values empty.
keys_body() {
    sort -u "$1" | sed 's/\\/\\\\/g; s/"/\\"/g' | awk 'NF { printf "%s  \"%s\": \"\"", (n++ ? ",\n" : ""), $0 }'
}

# render_state CURRENT PREVIOUS DISABLED OWNED-SUMS DIRS > FILE
render_state() {
    printf '{\n "v": 1,\n "current": "%s",\n "previous": "%s",\n "disabled": "%s",\n "files": {\n' \
        "$1" "$2" "$3"
    pairs_body "$4"
    printf '\n },\n "dirs": {\n'
    keys_body "$5"
    printf '\n }\n}\n'
}

# Replace installed.json with a rendered file, atomically.
commit_state() {
    [ ! -L "$STATE.tmp.$$" ] || die 65 "$STATE.tmp.$$ is a link"
    cp "$1" "$STATE.tmp.$$"
    mv -f "$STATE.tmp.$$" "$STATE"
}

# write_pending OP FROM TO DISABLED INVENTORY DIRS TEMPS REMOVE HOSTS FINAL:
# the intent record, written atomically before the command's first change.
# FINAL is the SHA-256 of the installed.json the command will commit ("" for
# uninstall); `phase` starts empty (see mark_hosts_done).
write_pending() {
    [ ! -L "$PENDING.tmp" ] && [ ! -d "$PENDING.tmp" ] || die 65 "$PENDING.tmp is not a plain file"
    {
        printf '{\n "v": 1,\n "op": "%s",\n "from": "%s",\n "to": "%s",\n "disabled": "%s",\n' \
            "$1" "$2" "$3" "$4"
        printf ' "final": "%s",\n "phase": "",\n' "${10}"
        printf ' "inventory": {\n'
        pairs_body "$5"
        printf '\n },\n "dirs": {\n'
        keys_body "$6"
        printf '\n },\n "temps": {\n'
        pairs_body "$7"
        printf '\n },\n "remove": {\n'
        pairs_body "$8"
        printf '\n },\n "host_configs": {\n'
        keys_body "$9"
        printf '\n }\n}\n'
    } > "$PENDING.tmp"
    mv -f "$PENDING.tmp" "$PENDING"
}

# Record in pending.json, atomically, that every host block it lists is
# removed: only this proves host cleanup happened.
mark_hosts_done() {
    [ ! -L "$PENDING.tmp" ] && [ ! -d "$PENDING.tmp" ] || die 65 "$PENDING.tmp is not a plain file"
    cp "$PENDING" "$PENDING.tmp"
    plutil -replace phase -string hosts-done -- "$PENDING.tmp" || die 70 "cannot record the host phase"
    mv -f "$PENDING.tmp" "$PENDING"
}

# The directories on the owned files' paths, from each version directory
# down: with the roots install created, the only directories ever pruned.
owned_dirs() {
    awk '{
        n = split($2, part, "/")
        path = part[1] "/" part[2]
        for (i = 3; i < n; i++) { path = path "/" part[i]; print path }
    }' "$1" | sort -u
}

# rmdir the listed directories deepest first; a non-empty one stays.
prune() {
    awk -F/ 'NF { print NF " " $0 }' "$1" | sort -rn | cut -d' ' -f2- |
        while IFS= read -r dir; do
            [ -L "$PREFIX/$dir" ] || rmdir "$PREFIX/$dir" 2>/dev/null || true
        done
}

# The lines of one component (core, semantic or learning) from sums lines
# whose paths are relative to the prefix.
component_lines() {
    awk -v want="$1" '{
        path = $2
        sub(/^lib\/context-foundry\/[^\/]*\//, "", path)
        part = "core"
        if (path ~ /^(libexec\/foundry-embed$|scripts\/embed-worker-bundle\.sh$|FoundryEmbed\.app\/|FoundryEmbed\.entitlements\.plist$|profiles\/semantic-)/)
            part = "semantic"
        else if (path ~ /^(libexec\/foundry-learn$|scripts\/learn-worker-bundle\.sh$|FoundryLearn\.app\/|FoundryLearn\.entitlements\.plist$|profiles\/learning-)/)
            part = "learning"
        if (part == want) print
    }' "$2"
}

# --- layout --------------------------------------------------------------

check_bin_link() {
    if [ -L "$PREFIX/bin/foundry" ]; then
        [ "$(readlink "$PREFIX/bin/foundry")" = "$BIN_LINK_TARGET" ] ||
            die 65 "$PREFIX/bin/foundry is a link this installer did not make; refusing to replace it"
    elif [ -e "$PREFIX/bin/foundry" ]; then
        die 65 "$PREFIX/bin/foundry is not this installer's link; refusing to replace it"
    fi
}

# The fixed part of the layout: real directories, plain record files and
# the two owned links in the installer's form.
check_layout() {
    for dir in bin lib lib/context-foundry lib/context-foundry/.lock; do
        [ ! -L "$PREFIX/$dir" ] || die 65 "$PREFIX/$dir is a symbolic link; refusing to operate through it"
    done
    for file in "$STATE" "$PENDING"; do
        [ ! -L "$file" ] || die 65 "$file is a symbolic link"
    done
    check_bin_link
    if [ -L "$LIB/current" ]; then
        linked=$(readlink "$LIB/current")
        case "$linked" in
            "" | *[!A-Za-z0-9.+-]* | . | ..) die 65 "$LIB/current points at '$linked', not a version directory" ;;
        esac
        [ ! -L "$LIB/$linked" ] || die 65 "$LIB/$linked is a symbolic link"
    elif [ -e "$LIB/current" ]; then
        die 65 "$LIB/current is not this installer's link"
    fi
}

# After recovery the `current` link and installed.json must agree: a link to
# a version no record names is refused, never adopted.
check_current() {
    if [ -L "$LIB/current" ]; then
        linked=$(readlink "$LIB/current")
        [ -f "$STATE" ] || die 65 "$LIB/current points at $linked, which installed.json does not record"
        recorded=$(json_value "$STATE" current)
        [ "$linked" = "$recorded" ] ||
            die 65 "$LIB/current points at $linked, but installed.json records $recorded"
    elif [ -e "$LIB/current" ]; then
        die 65 "$LIB/current is not this installer's link"
    elif [ -f "$STATE" ]; then
        die 65 "installed.json records $(json_value "$STATE" current), but $LIB/current is missing"
    fi
}

# Whether every directory on $1's path, from its version directory down, is
# a real directory and not a link. Shell tests only, so it is cheap per file.
real_dirs() {
    rest=${1#lib/context-foundry/}
    dir=lib/context-foundry
    while :; do
        case "$rest" in
            */*) ;;
            *) return 0 ;;
        esac
        dir=$dir/${rest%%/*}
        rest=${rest#*/}
        [ ! -L "$PREFIX/$dir" ] || return 1
    done
}

# Every directory on the listed files' paths (sums file $1) must be real.
check_owned_dirs() {
    owned_dirs "$1" > "$TMP/layout-dirs"
    while IFS= read -r dir; do
        [ ! -L "$PREFIX/$dir" ] || die 65 "$PREFIX/$dir is a symbolic link; refusing to follow it"
    done < "$TMP/layout-dirs"
}

# Split sums lines into TMP/match (the file still has its recorded hash)
# and TMP/differ (modified or missing). Callers checked the layout.
check_owned() {
    (cd "$PREFIX" && shasum -a 256 -c "$1" 2>/dev/null) > "$TMP/check" || true
    sed -n 's/: OK$//p' "$TMP/check" > "$TMP/match"
    grep -v ': OK$' "$TMP/check" | sed 's/: [^:]*$//' > "$TMP/differ" || true
}

# Delete the listed files that still match, each only if no directory on
# its path became a link; report the modified ones as kept.
remove_owned() {
    check_owned_dirs "$1"
    check_owned "$1"
    REMOVED=0
    GONE=0
    while IFS= read -r path; do
        real_dirs "$path" || die 65 "a directory on $PREFIX/$path became a link during removal; stopped"
        if [ -L "$PREFIX/$path" ]; then
            echo "kept (replaced by a link since install): $PREFIX/$path"
            continue
        fi
        rm -f "$PREFIX/$path"
        REMOVED=$((REMOVED + 1))
    done < "$TMP/match"
    while IFS= read -r path; do
        if [ -e "$PREFIX/$path" ] || [ -L "$PREFIX/$path" ]; then
            echo "kept (modified since install): $PREFIX/$path"
        else
            GONE=$((GONE + 1))
        fi
    done < "$TMP/differ"
}

# --- lock and recovery ---------------------------------------------------

# One lifecycle command at a time: LOCK holds the holder's pid and start
# time. A lock whose holder is gone (or whose pid now names another
# process) is reclaimed; a live holder is exit 75.
take_lock() {
    if ! mkdir "$LOCK" 2>/dev/null; then
        [ -d "$LOCK" ] && [ ! -L "$LOCK" ] || die 65 "$LOCK is not a real directory; refusing to read or remove it"
        if [ -L "$LOCK/owner" ] || { [ -e "$LOCK/owner" ] && [ ! -f "$LOCK/owner" ]; }; then
            die 65 "$LOCK/owner is not a plain file; refusing to read or remove it"
        fi
        holder=$(sed -n 1p "$LOCK/owner" 2>/dev/null) || holder=""
        started=$(sed -n 2p "$LOCK/owner" 2>/dev/null) || started=""
        case "$holder" in
            "" | *[!0-9]*)
                die 75 "$LOCK has no holder yet (being taken, or left by a crash right after mkdir); remove it if no lifecycle command runs"
                ;;
        esac
        live=$(ps -o lstart= -p "$holder" 2>/dev/null) || live=""
        if [ -n "$live" ] && [ "$live" = "$started" ]; then
            die 75 "another lifecycle command (pid $holder) holds $LOCK"
        fi
        rm -f "$LOCK/owner"
        rmdir "$LOCK" 2>/dev/null || die 75 "cannot reclaim the stale lock $LOCK"
        mkdir "$LOCK" 2>/dev/null || die 75 "another lifecycle command took $LOCK"
        echo "reclaimed the lock of pid $holder, which is gone"
    fi
    LOCKED=1
    printf '%s\n%s\n' "$$" "$(ps -o lstart= -p $$)" > "$LOCK/owner"
}

# Remove the temporaries an interrupted command recorded: a link only if it
# still names its recorded target, a file only if it has its recorded hash.
remove_temps() {
    sums_of "$PENDING" temps > "$TMP/temps"
    while read -r value path; do
        case "$value" in
            link:*)
                if [ -L "$PREFIX/$path" ] && [ "$(readlink "$PREFIX/$path")" = "${value#link:}" ]; then
                    rm -f "$PREFIX/$path"
                fi
                ;;
            *)
                if [ -f "$PREFIX/$path" ] && [ ! -L "$PREFIX/$path" ] && [ "$(sha "$PREFIX/$path")" = "$value" ]; then
                    rm -f "$PREFIX/$path"
                fi
                ;;
        esac
    done < "$TMP/temps"
}

# Resolve an interrupted command from pending.json (header). A record whose
# installed.json is already committed is finalized; otherwise switches are
# completed or rolled back, and disable and uninstall are completed. Nothing
# happens unless `current` is absent where the operation allows it or names
# exactly one of its recorded endpoints.
recover() {
    [ -f "$PENDING" ] || return 0
    op=$(json_value "$PENDING" op)
    from=$(json_value "$PENDING" from)
    to=$(json_value "$PENDING" to)
    intended=$(json_value "$PENDING" disabled)
    final=$(json_value "$PENDING" final)
    linked=""
    [ ! -L "$LIB/current" ] || linked=$(readlink "$LIB/current")
    case "$op" in
        install | upgrade | rollback) allowed="$from $to" ;;
        disable-semantic | disable-learning) allowed=$from ;;
        uninstall) allowed="$from " ;;
        *) die 65 "$PENDING records an unknown operation '$op'" ;;
    esac
    case " $allowed " in
        *" $linked "*) ;;
        *) die 65 "$LIB/current points at '$linked', which the interrupted $op does not record; nothing was changed" ;;
    esac
    if [ -n "$final" ] && [ -f "$STATE" ] && [ "$(sha "$STATE")" = "$final" ]; then
        # Committed: only the record was left. Links must agree before it
        # goes; an edit made since is reported, never undone.
        if [ "$op" = install ] || [ "$op" = upgrade ] || [ "$op" = rollback ]; then
            [ "$linked" = "$to" ] || die 65 "installed.json records $to but $LIB/current does not; nothing was changed"
            link_bin
            sums_of "$PENDING" inventory > "$TMP/inventory"
            check_owned "$TMP/inventory"
            while IFS= read -r path; do
                [ ! -e "$PREFIX/$path" ] || echo "kept (modified since install): $PREFIX/$path"
            done < "$TMP/differ"
        fi
        echo "recovered the interrupted $op: installed.json was committed; finalized it"
        remove_temps
        rm -f "$PENDING"
        return 0
    fi
    case "$op" in
        install | upgrade | rollback)
            sums_of "$PENDING" inventory > "$TMP/inventory"
            keys_of "$PENDING" dirs > "$TMP/pending-dirs"
            forward=""
            if [ "$linked" = "$to" ]; then
                check_owned_dirs "$TMP/inventory"
                check_owned "$TMP/inventory"
                [ -s "$TMP/differ" ] || forward=1
            fi
            if [ -n "$forward" ]; then
                : > "$TMP/owned"
                : > "$TMP/dirs"
                if [ -f "$STATE" ]; then
                    sums_of "$STATE" files > "$TMP/owned"
                    keys_of "$STATE" dirs > "$TMP/dirs"
                fi
                cat "$TMP/inventory" >> "$TMP/owned"
                cat "$TMP/pending-dirs" >> "$TMP/dirs"
                link_bin
                render_state "$to" "$from" "$intended" "$TMP/owned" "$TMP/dirs" > "$TMP/state"
                commit_state "$TMP/state"
                echo "recovered the interrupted $op: completed it ($to is current)"
            else
                if [ -n "$from" ]; then
                    [ -d "$LIB/$from" ] && [ ! -L "$LIB/$from" ] ||
                        die 65 "cannot roll back the interrupted $op: $LIB/$from is missing"
                    [ "$linked" = "$from" ] || switch_current "$from"
                else
                    if [ -n "$linked" ] && [ "$linked" = "$to" ]; then
                        rm -f "$LIB/current"
                    fi
                    if [ ! -f "$STATE" ] && [ -L "$PREFIX/bin/foundry" ]; then
                        check_bin_link
                        rm -f "$PREFIX/bin/foundry"
                    fi
                fi
                if [ "$op" != rollback ]; then
                    remove_owned "$TMP/inventory"
                fi
                prune "$TMP/pending-dirs"
                # The roots an interrupted first install created stay
                # recorded for the install that follows.
                while IFS= read -r dir; do
                    case "$dir" in
                        lib | bin | lib/context-foundry) [ ! -d "$PREFIX/$dir" ] || echo "$dir" >> "$TMP/roots" ;;
                    esac
                done < "$TMP/pending-dirs"
                echo "recovered the interrupted $op: rolled it back (current: ${from:-nothing installed})"
            fi
            ;;
        disable-semantic | disable-learning)
            finish_disable
            echo "recovered the interrupted $op: completed it"
            ;;
        uninstall)
            finish_uninstall
            echo "recovered the interrupted uninstall: completed it"
            ;;
    esac
    remove_temps
    rm -f "$PENDING"
}

# Every command: refuse an unexpected layout, take the lock, resolve an
# interrupted command (never while an installed executable runs), then
# require the link and the record to agree.
begin() {
    check_layout
    mkdir -p "$LIB"
    take_lock
    check_layout
    [ ! -f "$PENDING" ] || refuse_if_busy
    recover
    check_current
    : > "$TMP/owned"
    : > "$TMP/dirs"
    if [ -f "$STATE" ]; then
        sums_of "$STATE" files > "$TMP/owned"
        keys_of "$STATE" dirs > "$TMP/dirs"
        check_owned_dirs "$TMP/owned"
    fi
}

# --- processes, links ----------------------------------------------------

# Refuse while any process of this user runs code from under the prefix,
# matched by the canonical path of its mapped executables (`lsof` text
# entries), never by process name. The prefix is printable ASCII, so lsof
# prints it unescaped, and it reaches awk through the environment.
refuse_if_busy() {
    lsof -nP -d txt -F pn > "$TMP/lsof" 2>/dev/null || true
    [ -s "$TMP/lsof" ] || die 70 "cannot list running executables (lsof)"
    BUSY_ROOT="$PREFIX/" awk '
        BEGIN { root = ENVIRON["BUSY_ROOT"] }
        /^p/ { pid = substr($0, 2); next }
        /^n/ { name = substr($0, 2); if (index(name, root) == 1) print "  pid " pid ": " name }
    ' "$TMP/lsof" | sort -u > "$TMP/busy"
    if [ -s "$TMP/busy" ]; then
        echo "install.sh: refusing $COMMAND: these processes run executables under $PREFIX; stop them first:" >&2
        cat "$TMP/busy" >&2
        exit 75
    fi
}

switch_current() {
    ln -s "$1" "$LIB/.current.tmp.$$"
    # BSD mv: -h renames over the link instead of moving into its directory.
    mv -h -f "$LIB/.current.tmp.$$" "$LIB/current"
}

link_bin() {
    mkdir -p "$PREFIX/bin"
    check_bin_link
    [ ! -L "$PREFIX/bin/foundry" ] || return 0
    ln -s "$BIN_LINK_TARGET" "$PREFIX/bin/.foundry.tmp.$$"
    mv -h -f "$PREFIX/bin/.foundry.tmp.$$" "$PREFIX/bin/foundry"
}

# Tests only (header): interrupt or pause at the named point.
test_hook() {
    [ -n "${CF_TEST_HOOK:-}" ] || return 0
    case "${TARGET_VERSION:-}" in
        *-test.*) ;;
        *) return 0 ;;
    esac
    case "$CF_TEST_HOOK" in
        "kill:$1") kill -9 $$ ;;
        "wait:$1:"*)
            barrier=${CF_TEST_HOOK#"wait:$1:"}
            : > "$barrier/arrived"
            waited=0
            while [ ! -e "$barrier/go" ]; do
                sleep 0.1
                waited=$((waited + 1))
                [ "$waited" -lt 600 ] || die 70 "test hook $CF_TEST_HOOK timed out"
            done
            ;;
    esac
}

# --- packages ------------------------------------------------------------

# Unpack the package into the private temporary directory and verify it
# against its PACKAGE.json. Sets PKG (the package root) and PKG_VERSION.
fetch_package() {
    mkdir "$TMP/pkg"
    tar -xzf "$PACKAGE" -C "$TMP/pkg" || die 65 "cannot unpack $PACKAGE"
    (cd "$TMP/pkg" && find . ! -type f ! -type d) > "$TMP/special"
    [ ! -s "$TMP/special" ] || die 65 "the package holds a non-regular file: $(head -1 "$TMP/special")"
    [ "$(cd "$TMP/pkg" && ls -A | wc -l | tr -d ' ')" = 1 ] ||
        die 65 "the package must hold exactly one top-level directory"
    top=$(cd "$TMP/pkg" && ls -A)
    case "$top" in
        context-foundry-*-macos-arm64) ;;
        *) die 65 "the package must hold one context-foundry-<version>-macos-arm64 directory" ;;
    esac
    PKG=$TMP/pkg/$top
    [ -f "$PKG/PACKAGE.json" ] || die 65 "the package has no PACKAGE.json"
    PKG_VERSION=$(json_value "$PKG/PACKAGE.json" version)
    case "$PKG_VERSION" in
        "" | *[!A-Za-z0-9.+-]* | . | ..) die 65 "invalid package version '$PKG_VERSION'" ;;
    esac
    [ "$top" = "context-foundry-$PKG_VERSION-macos-arm64" ] || die 65 "$top does not match version $PKG_VERSION"
    target=$(json_value "$PKG/PACKAGE.json" target)
    [ "$target" = aarch64-apple-darwin ] || die 65 "the package targets '$target', not aarch64-apple-darwin"
    [ "$(uname -s) $(uname -m)" = "Darwin arm64" ] || die 65 "this host is not macOS arm64"
    sums_of "$PKG/PACKAGE.json" files > "$TMP/manifest"
    if grep -v '^[0-9a-f]\{64\}  [A-Za-z0-9._+/-]*$' "$TMP/manifest" > "$TMP/odd"; then
        die 65 "PACKAGE.json lists an invalid entry: $(head -1 "$TMP/odd")"
    fi
    cut -c67- "$TMP/manifest" | sort > "$TMP/listed"
    (cd "$PKG" && find . -type f ! -path ./PACKAGE.json | sed 's|^\./||' | sort) > "$TMP/present"
    cmp -s "$TMP/listed" "$TMP/present" ||
        die 65 "package files differ from PACKAGE.json: $(diff "$TMP/listed" "$TMP/present" | sed -n 2p)"
    (cd "$PKG" && shasum -a 256 -c "$TMP/manifest") > "$TMP/verify" 2>&1 ||
        die 65 "package file hashes do not match PACKAGE.json: $(grep -v ': OK$' "$TMP/verify" | head -1)"
    TARGET_VERSION=$PKG_VERSION
    echo "verified $(grep -c . "$TMP/listed") files of $top against PACKAGE.json"
}

# build_semantic PROFILE [EXTRA-READ-DIR]...: build the ad-hoc-signed
# semantic bundle, the installed profile copy and the extra-read record (one
# canonical directory per line) in TMP/b, laid out as in the version
# directory.
build_semantic() {
    source_profile=$1
    shift
    [ -x "$PKG/libexec/foundry-embed" ] || die 69 "this package has no semantic worker (package it --with-semantic)"
    mkdir -p "$TMP/b/profiles"
    : > "$TMP/b/profiles/semantic-extra-read.txt"
    given=$#
    for dir in "$@"; do
        canonical=$(cd "$dir" && pwd -P) || die 66 "no directory $dir"
        check_value --semantic-extra-read "$canonical"
        printf '%s\n' "$canonical" >> "$TMP/b/profiles/semantic-extra-read.txt"
        set -- "$@" --extra-read "$canonical"
    done
    shift "$given"
    /bin/sh "$PKG/scripts/embed-worker-bundle.sh" --binary "$PKG/libexec/foundry-embed" \
        --out "$TMP/b/FoundryEmbed.app" --profile "$source_profile" "$@" > "$TMP/bundle.out" ||
        die 1 "embed-worker-bundle.sh failed for $source_profile"
    installed_profile "$source_profile" semantic-profile.json FoundryEmbed.app foundry-embed
}

# build_semantic with the --semantic-extra-read values of the option pairs.
build_semantic_from_options() {
    source_profile=$1
    shift
    pairs=$#
    option=""
    for word in "$@"; do
        if [ -z "$option" ]; then
            option=$word
        else
            [ "$option" != --semantic-extra-read ] || set -- "$@" "$word"
            option=""
        fi
    done
    shift "$pairs"
    build_semantic "$source_profile" "$@"
}

# build_semantic with the extra-read directories a previous version recorded.
build_semantic_from_record() {
    source_profile=$1
    record=$2
    set --
    if [ -f "$record" ]; then
        while IFS= read -r dir; do
            [ -z "$dir" ] || set -- "$@" "$dir"
        done < "$record"
    fi
    build_semantic "$source_profile" "$@"
}

build_learning() {
    [ -x "$PKG/libexec/foundry-learn" ] || die 69 "this package has no learning worker (package it --with-learning)"
    mkdir -p "$TMP/b/profiles"
    /bin/sh "$PKG/scripts/learn-worker-bundle.sh" --binary "$PKG/libexec/foundry-learn" \
        --out "$TMP/b/FoundryLearn.app" --profile "$1" > "$TMP/bundle.out" ||
        die 1 "learn-worker-bundle.sh failed for $1"
    installed_profile "$1" learning-profile.json FoundryLearn.app foundry-learn
}

# installed_profile SOURCE NAME APP EXECUTABLE: the installed copy names the
# bundle where it will be installed.
installed_profile() {
    exe_sha=$(sha "$TMP/b/$3/Contents/MacOS/$4")
    cp "$1" "$TMP/b/profiles/$2"
    plutil -replace worker.bundle -string "$LIB/$PKG_VERSION/$3" -- "$TMP/b/profiles/$2" &&
        plutil -replace worker.executable_sha256 -string "$exe_sha" -- "$TMP/b/profiles/$2" ||
        die 65 "cannot fill worker.bundle/executable_sha256 in a copy of $1"
    echo "$2: $LIB/current/profiles/$2 (bundle $LIB/$PKG_VERSION/$3, executable sha256 $exe_sha; from $1)"
}

# The target version's files as sums lines relative to the prefix: the
# verified PACKAGE.json hashes of the files still in the package, PACKAGE.json
# itself, and the outputs built in TMP/b.
target_inventory() {
    while read -r sum path; do
        [ ! -f "$PKG/$path" ] || echo "$sum  $path"
    done < "$TMP/manifest"
    echo "$(sha "$PKG/PACKAGE.json")  PACKAGE.json"
    if [ -d "$TMP/b" ]; then
        (cd "$TMP/b" && find . -type f | sed 's|^\./||' | sort) > "$TMP/built"
        if grep -v '^[A-Za-z0-9._+/-]*$' "$TMP/built" > "$TMP/odd"; then
            die 65 "unexpected built path: $(head -1 "$TMP/odd")"
        fi
        (cd "$TMP/b" && tr '\n' '\0' < "$TMP/built" | xargs -0 shasum -a 256)
    fi
}

# switch_to OP FROM TO DISABLED: with TMP/inventory (the target's files,
# relative to the prefix), TMP/new-dirs (the directories this command
# creates) and, for install and upgrade, the prepared target in TMP: record
# the intent, place the target, switch the links, commit installed.json,
# then drop the intent.
switch_to() {
    cat "$TMP/owned" "$TMP/inventory" > "$TMP/next-owned"
    cat "$TMP/dirs" "$TMP/new-dirs" > "$TMP/next-dirs"
    render_state "$3" "$2" "$4" "$TMP/next-owned" "$TMP/next-dirs" > "$TMP/next-state"
    {
        echo "link:$3  lib/context-foundry/.current.tmp.$$"
        echo "link:$BIN_LINK_TARGET  bin/.foundry.tmp.$$"
        echo "$(sha "$TMP/next-state")  lib/context-foundry/installed.json.tmp.$$"
    } > "$TMP/temps"
    write_pending "$1" "$2" "$3" "$4" "$TMP/inventory" "$TMP/new-dirs" "$TMP/temps" "$TMP/empty" \
        "$TMP/empty" "$(sha "$TMP/next-state")"
    if [ "$1" != rollback ]; then
        mv "$PKG" "$LIB/$3"
        if [ -d "$TMP/b" ]; then
            for entry in "$TMP/b"/*; do
                mv "$entry" "$LIB/$3/"
            done
        fi
    fi
    test_hook after-place
    switch_current "$3"
    test_hook after-current
    link_bin
    test_hook after-links
    commit_state "$TMP/next-state"
    test_hook after-commit
    rm -f "$PENDING"
}

# Print the versions the installed foundry reports (bootstrap inspection
# writes nothing).
readback() {
    "$PREFIX/bin/foundry" bootstrap --root "$PREFIX" > "$TMP/readback.json" ||
        die 70 "the installed foundry did not run"
    line="installed:"
    for key in core store_schema embed_protocol learn_protocol predict_protocol foundry_embed foundry_learn; do
        value=$(plutil -extract "versions.$key" raw -o - -- "$TMP/readback.json" 2>/dev/null) || value=none
        line="$line $key=$value"
    done
    echo "$line"
}

schema_of() {
    plutil -extract versions.store_schema raw -o - -- "$LIB/$1/PACKAGE.json" 2>/dev/null || echo unknown
}

check_store() {
    if "$PREFIX/bin/foundry" --store "$1" status > /dev/null 2> "$TMP/status.err"; then
        echo "store $1: schema $NEW_SCHEMA, no upgrade-store needed"
    elif grep -q '"upgrade_required"' "$TMP/status.err"; then
        echo "store $1: needs 'foundry --store $1 upgrade-store --to $NEW_SCHEMA' (not run here)"
    else
        echo "store $1: not checked: $(tail -1 "$TMP/status.err")"
    fi
}

record_host_config() {
    case "$1" in
        /*) echo "$1" ;;
        *) echo "$(pwd -P)/$1" ;;
    esac >> "$TMP/hosts"
}

# Explicit profile options re-enable a disabled component.
enable() {
    DISABLED=$(printf '%s' "$DISABLED" | tr ' ' '\n' | grep -vx "$1" | tr '\n' ' ' | sed 's/ $//') || true
}

disabled() {
    case " $DISABLED " in *" $1 "*) return 0 ;; *) return 1 ;; esac
}

# A descriptor v1 (MLX worker) semantic profile: 009 T004's core refuses it
# at load (`profile_unsupported`); only v2 (llama.cpp) profiles are served.
v1_profile() {
    [ "$(plutil -extract v raw -o - -- "$1" 2>/dev/null)" = 1 ] ||
        [ "$(plutil -extract descriptor.v raw -o - -- "$1" 2>/dev/null)" = 1 ]
}

# disable_state PART-SUMS DISABLED > FILE: installed.json without PART's
# files, with the intended disabled set (the recorded directories stay).
disable_state() {
    sums_of "$STATE" files > "$TMP/owned"
    keys_of "$STATE" dirs > "$TMP/dirs"
    grep -v -x -F -f "$1" "$TMP/owned" > "$TMP/rest" || true
    render_state "$(json_value "$STATE" current)" "$(json_value "$STATE" previous)" "$2" \
        "$TMP/rest" "$TMP/dirs"
}

# Remove the component files pending.json lists and record the rest.
finish_disable() {
    sums_of "$PENDING" remove > "$TMP/part"
    disable_state "$TMP/part" "$(json_value "$PENDING" disabled)" > "$TMP/state"
    remove_owned "$TMP/part"
    prune "$TMP/dirs"
    commit_state "$TMP/state"
}

# Remove the host blocks, the owned files and links pending.json lists, and
# installed.json; the caller prunes the recorded directories. Host cleanup
# is skipped only when pending.json's phase proves it happened, and it needs
# the installed foundry: without it, nothing is changed (exit 66).
finish_uninstall() {
    keys_of "$PENDING" host_configs > "$TMP/hosts"
    keys_of "$PENDING" dirs > "$TMP/uninstall-dirs"
    if [ -s "$TMP/hosts" ] && [ "$(json_value "$PENDING" phase)" != hosts-done ]; then
        while IFS= read -r config; do
            [ -x "$PREFIX/bin/foundry" ] ||
                die 66 "cannot remove the owned block from $config: $PREFIX/bin/foundry is missing or not executable; nothing more was changed"
            "$PREFIX/bin/foundry" connect --remove-config "$config" > "$TMP/remove.json" ||
                die 1 "could not remove the owned block from $config; nothing more was uninstalled"
            if grep -q '"removed": true' "$TMP/remove.json"; then
                echo "host config $config: owned block removed, original bytes restored"
            else
                echo "host config $config: no owned block"
            fi
        done < "$TMP/hosts"
        mark_hosts_done
        test_hook after-hosts
    fi
    sums_of "$PENDING" remove > "$TMP/all"
    remove_owned "$TMP/all"
    check_bin_link
    [ ! -L "$PREFIX/bin/foundry" ] || rm -f "$PREFIX/bin/foundry"
    [ ! -L "$LIB/current" ] || rm -f "$LIB/current"
    rm -f "$STATE"
    prune "$TMP/uninstall-dirs"
}

# --- commands ------------------------------------------------------------

: > "$TMP/roots"
case "$COMMAND" in
install)
    # The directories this install creates are recorded, and only those are
    # ever pruned; the prefix itself is the operator's.
    for dir in lib lib/context-foundry bin; do
        [ -e "$PREFIX/$dir" ] || [ -L "$PREFIX/$dir" ] || echo "$dir" >> "$TMP/roots"
    done
    begin
    [ ! -e "$STATE" ] || die 73 "Context Foundry is already installed under $PREFIX; use upgrade"
    fetch_package
    [ ! -e "$LIB/$PKG_VERSION" ] && [ ! -L "$LIB/$PKG_VERSION" ] || die 73 "$LIB/$PKG_VERSION already exists"
    [ -z "$SEMANTIC_PROFILE" ] || build_semantic_from_options "$SEMANTIC_PROFILE" "$@"
    [ -z "$LEARNING_PROFILE" ] || build_learning "$LEARNING_PROFILE"
    target_inventory | sed "s|  |  lib/context-foundry/$PKG_VERSION/|" > "$TMP/inventory"
    { cat "$TMP/roots"; owned_dirs "$TMP/inventory"; } > "$TMP/new-dirs"
    switch_to install "" "$PKG_VERSION" ""
    echo "installed $PKG_VERSION: $PREFIX/bin/foundry -> $LIB/$PKG_VERSION/bin/foundry"
    readback
    ;;
upgrade)
    begin
    [ -f "$STATE" ] || die 66 "nothing is installed under $PREFIX; use install"
    OLD=$(json_value "$STATE" current)
    DISABLED=$(json_value "$STATE" disabled)
    refuse_if_busy
    fetch_package
    [ "$PKG_VERSION" != "$OLD" ] || die 73 "$PKG_VERSION is already the current version"
    [ ! -e "$LIB/$PKG_VERSION" ] && [ ! -L "$LIB/$PKG_VERSION" ] ||
        die 73 "$PKG_VERSION is already installed; rollback switches between the current and the previous version"
    [ -z "$SEMANTIC_PROFILE" ] || enable semantic
    [ -z "$LEARNING_PROFILE" ] || enable learning
    # An installed descriptor v1 profile is never carried forward: the core
    # upgrade goes ahead with semantic retrieval disabled by name, and a v2
    # profile is asked for. The old version keeps its bundle and profile, so
    # a rollback is that binary with its own profile.
    if [ -z "$SEMANTIC_PROFILE" ] && ! disabled semantic &&
        [ -f "$LIB/$OLD/profiles/semantic-profile.json" ] &&
        v1_profile "$LIB/$OLD/profiles/semantic-profile.json"; then
        DISABLED=$(echo "$DISABLED semantic" | sed 's/^ //')
        echo "semantic: disabled: $OLD's installed profile is descriptor v1 (the MLX worker), which $PKG_VERSION refuses (profile_unsupported); the core upgrade goes ahead without the semantic worker, and stores keep their semantic cache until 'foundry semantic purge'"
        echo "semantic: to enable it again, give a v2 (llama.cpp) profile with --semantic-profile FILE to an upgrade"
    fi
    # Disabled components stay disabled: their package files are not placed.
    if disabled semantic; then
        rm -f "$PKG/libexec/foundry-embed" "$PKG/scripts/embed-worker-bundle.sh"
    fi
    if disabled learning; then
        rm -f "$PKG/libexec/foundry-learn" "$PKG/scripts/learn-worker-bundle.sh"
    fi
    if [ -n "$SEMANTIC_PROFILE" ]; then
        build_semantic_from_options "$SEMANTIC_PROFILE" "$@"
    elif [ -f "$LIB/$OLD/profiles/semantic-profile.json" ] && ! disabled semantic; then
        if [ -x "$PKG/libexec/foundry-embed" ]; then
            echo "semantic: rebuilding the bundle $OLD had"
            build_semantic_from_record "$LIB/$OLD/profiles/semantic-profile.json" \
                "$LIB/$OLD/profiles/semantic-extra-read.txt"
        else
            echo "semantic: $PKG_VERSION has no semantic worker; $OLD's bundle is not carried forward"
        fi
    fi
    if [ -n "$LEARNING_PROFILE" ]; then
        build_learning "$LEARNING_PROFILE"
    elif [ -f "$LIB/$OLD/profiles/learning-profile.json" ] && ! disabled learning; then
        if [ -x "$PKG/libexec/foundry-learn" ]; then
            echo "learning: rebuilding the bundle $OLD had"
            build_learning "$LIB/$OLD/profiles/learning-profile.json"
        else
            echo "learning: $PKG_VERSION has no learning worker; $OLD's bundle is not carried forward"
        fi
    fi
    target_inventory | sed "s|  |  lib/context-foundry/$PKG_VERSION/|" > "$TMP/inventory"
    owned_dirs "$TMP/inventory" > "$TMP/new-dirs"
    # The cutover: owners are checked again now that preparation is done.
    test_hook before-cutover
    refuse_if_busy
    check_layout
    switch_to upgrade "$OLD" "$PKG_VERSION" "$DISABLED"
    echo "upgraded $OLD -> $PKG_VERSION: $PREFIX/bin/foundry -> $LIB/$PKG_VERSION/bin/foundry ($OLD stays installed for rollback)"
    old_schema=$(schema_of "$OLD")
    NEW_SCHEMA=$(schema_of "$PKG_VERSION")
    if [ "$old_schema" = "$NEW_SCHEMA" ]; then
        echo "store schema unchanged ($NEW_SCHEMA): no store needs foundry upgrade-store"
    else
        echo "store schema $old_schema -> $NEW_SCHEMA: each store $OLD wrote needs 'foundry --store DIR upgrade-store --to $NEW_SCHEMA' (not run here)"
    fi
    for_each --store check_store "$@"
    readback
    ;;
rollback)
    begin
    [ -f "$STATE" ] || die 66 "nothing is installed under $PREFIX"
    CURRENT=$(json_value "$STATE" current)
    PREVIOUS=$(json_value "$STATE" previous)
    TARGET_VERSION=$PREVIOUS
    DISABLED=$(json_value "$STATE" disabled)
    [ -n "$PREVIOUS" ] || die 69 "no previous version to roll back to"
    [ -d "$LIB/$PREVIOUS" ] && [ ! -L "$LIB/$PREVIOUS" ] ||
        die 66 "the previous version directory $LIB/$PREVIOUS is missing"
    grep -F "  lib/context-foundry/$PREVIOUS/" "$TMP/owned" > "$TMP/inventory" || true
    check_owned "$TMP/inventory"
    [ ! -s "$TMP/differ" ] || die 65 "$PREVIOUS was modified since install, not switching: $(head -1 "$TMP/differ")"
    refuse_if_busy
    : > "$TMP/new-dirs"
    switch_to rollback "$CURRENT" "$PREVIOUS" "$DISABLED"
    echo "rolled back $CURRENT -> $PREVIOUS (rollback again returns to $CURRENT)"
    echo "a store already upgraded to a newer schema is refused by $PREVIOUS (unsupported_schema); restore its pre-upgrade copy to use it here"
    readback
    ;;
disable-semantic | disable-learning)
    begin
    [ -f "$STATE" ] || die 66 "nothing is installed under $PREFIX"
    PART=${COMMAND#disable-}
    CURRENT=$(json_value "$STATE" current)
    TARGET_VERSION=$CURRENT
    DISABLED=$(json_value "$STATE" disabled)
    disabled "$PART" || DISABLED=$(echo "$DISABLED $PART" | sed 's/^ //')
    refuse_if_busy
    component_lines "$PART" "$TMP/owned" > "$TMP/part"
    disable_state "$TMP/part" "$DISABLED" > "$TMP/state"
    echo "$(sha "$TMP/state")  lib/context-foundry/installed.json.tmp.$$" > "$TMP/temps"
    write_pending "$COMMAND" "$CURRENT" "$CURRENT" "$DISABLED" "$TMP/empty" "$TMP/empty" \
        "$TMP/temps" "$TMP/part" "$TMP/empty" "$(sha "$TMP/state")"
    finish_disable
    rm -f "$PENDING"
    echo "disabled $PART: removed $REMOVED owned files from every installed version; supplied profiles, scratch roots, models and checkpoints are untouched"
    ;;
uninstall)
    begin
    [ -f "$STATE" ] || die 66 "nothing is installed under $PREFIX"
    CURRENT=$(json_value "$STATE" current)
    TARGET_VERSION=$CURRENT
    refuse_if_busy
    : > "$TMP/hosts"
    for_each --host-config record_host_config "$@"
    if [ -s "$TMP/hosts" ] && [ ! -x "$PREFIX/bin/foundry" ]; then
        die 66 "cannot remove the owned block from $(head -1 "$TMP/hosts"): $PREFIX/bin/foundry is missing or not executable; nothing was changed"
    fi
    write_pending uninstall "$CURRENT" "" "" "$TMP/empty" "$TMP/dirs" "$TMP/empty" \
        "$TMP/owned" "$TMP/hosts" ""
    test_hook after-pending
    finish_uninstall
    rm -f "$PENDING"
    rm -f "$LOCK/owner"
    rmdir "$LOCK" 2>/dev/null || true
    LOCKED=""
    prune "$TMP/uninstall-dirs"
    echo "uninstalled from $PREFIX: removed $REMOVED owned files"
    [ "$GONE" = 0 ] || echo "$GONE owned files were already gone"
    echo "preserved: every store, cache, memory, dataset, checkpoint, scratch root and supplied profile (none lives in the installed files)"
    ;;
esac
