#!/bin/sh
# Run the frozen G2 retrieval benchmark (388 identifier-free questions over
# rust-lang/rust library/ and oh-my-pi) on a host that holds the owner's
# private inputs. The questions, harness, corpora and G1 parser stay outside
# Git; this script only drives them. The harness (g2.py) is frozen: one MCP
# `context` call per question at 2,048 tokens, scored by the G1 parser.
#
#   scripts/g2.sh index   OUT                      index both corpora into OUT/<corpus>-lex
#   scripts/g2.sh prepare OUT KIND PROFILE         clone OUT/<corpus>-lex to OUT/<corpus>-KIND
#                                                  and run `semantic prepare` to completion
#   scripts/g2.sh run     OUT NAME KIND [PROFILE] [-- MCP_FLAG...]
#                                                  run the harness on fresh clones of
#                                                  OUT/<corpus>-KIND into OUT/runs/NAME
#   scripts/g2.sh compare OUT BASE CAND K          paired exact one-sided McNemar test at
#                                                  alpha 0.05/K, K = candidates predeclared
#                                                  in the family (2026-10-08 froze K=2)
#
# NAME, KIND, BASE and CAND are single names: letters, digits, '.', '_' and '-',
# not starting with '.'. FOUNDRY names the foundry binary (default:
# target/release/foundry). G2_DATA (default ~/VSC_DEV/datasets) holds
# context-foundry-g2/questions.jsonl, context-foundry-g2-harness/g2.py and
# context-foundry-citymap/g1score.py; g2.py reads the corpora from
# ~/VSC_DEV/corpora/rust-1.99.0 and ~/VSC_DEV/oh-my-pi at the commits pinned in
# context-foundry-g2/FROZEN.txt. A profile run adds --development-isolation
# (worker bundles are ad-hoc signed). The harness token counter is built at its
# frozen path when missing.
set -eu
REPO=$(cd "$(dirname "$0")/.." && pwd -P)
FOUNDRY=${FOUNDRY:-$REPO/target/release/foundry}
DATA=${G2_DATA:-$HOME/VSC_DEV/datasets}
QUESTIONS=$DATA/context-foundry-g2/questions.jsonl
HARNESS=$DATA/context-foundry-g2-harness/g2.py
O200K=/private/tmp/cf-o200k/target/release/o200k_count

usage() { sed -n '2,26p' "$0" >&2; exit 64; }
fail() { echo "g2: $2" >&2; exit "$1"; }
root() {
    case $1 in
    rust) echo "$HOME/VSC_DEV/corpora/rust-1.99.0" ;;
    bun) echo "$HOME/VSC_DEV/oh-my-pi" ;;
    esac
}
need() { [ -e "$1" ] || fail 66 "missing $1"; }
# Names become path components: refuse separators, '.', '..' and hidden names.
name() {
    case $1 in
    '' | .* | *[!A-Za-z0-9._-]*) fail 64 "invalid name: '$1'" ;;
    esac
}
# One shell word that evaluates back to exactly $1. Pure parameter expansion: no
# echo escapes, no sed newline handling, and the closing quote protects trailing
# newlines from the caller's command substitution.
quote() {
    rest=$1 done=
    while :; do
        case $rest in
        *\'*) done="$done${rest%%\'*}'\\''"; rest=${rest#*\'} ;;
        *) break ;;
        esac
    done
    printf "'%s%s'" "$done" "$rest"
}
# Stores are large: clone on APFS, else copy. The destination must not exist,
# so a failed clone removes only what this call created.
copy() {
    [ ! -e "$2" ] || fail 73 "$2 exists"
    cp -c -R "$1" "$2" 2>/dev/null || { rm -rf "$2"; cp -R "$1" "$2"; }
}
counter() {
    [ -x "$O200K" ] && return
    dir=$(dirname "$(dirname "$(dirname "$O200K")")")
    mkdir -p "$dir/src"
    printf '%s\n' '[package]' 'name = "o200k_count"' 'version = "0.1.0"' 'edition = "2021"' \
        '[dependencies]' 'tiktoken-rs = "=0.12.1"' > "$dir/Cargo.toml"
    # The frozen protocol: "<len>\n<bytes>" frames in, one encode_ordinary count out.
    cat > "$dir/src/main.rs" <<'EOF'
use std::io::{BufRead, Read, Write};
fn main() {
    let bpe = tiktoken_rs::o200k_base_singleton();
    let mut input = std::io::stdin().lock();
    let mut out = std::io::stdout().lock();
    let mut line = String::new();
    while { line.clear(); input.read_line(&mut line).unwrap() > 0 } {
        let mut bytes = vec![0; line.trim().parse().unwrap()];
        input.read_exact(&mut bytes).unwrap();
        writeln!(out, "{}", bpe.encode_ordinary(std::str::from_utf8(&bytes).unwrap()).len()).unwrap();
        out.flush().unwrap();
    }
}
EOF
    (cd "$dir" && cargo build --release --quiet)
}

[ $# -ge 2 ] || usage
cmd=$1 out=$2
shift 2
need "$FOUNDRY"
case $cmd in
index)
    [ $# -eq 0 ] || usage
    mkdir -p "$out"
    for c in rust bun; do
        need "$(root $c)"
        [ ! -e "$out/$c-lex" ] || fail 73 "$out/$c-lex exists"
        "$FOUNDRY" --store "$out/$c-lex" index "$(root $c)" > "$out/index-$c.json"
    done
    ;;
prepare)
    [ $# -eq 2 ] || usage
    kind=$1 profile=$2
    name "$kind"
    for c in rust bun; do
        store=$out/$c-$kind
        [ -e "$store" ] || copy "$out/$c-lex" "$store"
        round=1
        while :; do
            report=$out/prepare-$c-$kind.$round.json
            status=0
            "$FOUNDRY" --store "$store" semantic prepare --profile "$profile" \
                --budget-seconds 7200 --development-isolation --cache-cap 4294967296 \
                > "$report" || status=$?
            [ $status -le 1 ] || exit $status
            python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); sys.exit(0 if d.get("partial") is False else 1)' "$report" && break
            round=$((round + 1))
            [ $round -le 20 ] || fail 75 "$c preparation did not complete"
        done
    done
    ;;
run)
    [ $# -ge 2 ] || usage
    run=$1 kind=$2
    shift 2
    name "$run"
    name "$kind"
    profile=
    if [ $# -gt 0 ] && [ "$1" != -- ]; then profile=$1; shift; fi
    if [ $# -gt 0 ]; then
        [ "$1" = -- ] || usage
        shift
    fi
    need "$QUESTIONS"; need "$HARNESS"; counter
    runs=$out/runs
    [ ! -e "$runs/$run" ] || fail 73 "$runs/$run exists"
    mkdir -p "$runs" "$out/bin" "$out/runstores"
    # g2.py names only --semantic-profile; extra MCP flags ride on a wrapper
    # whose binary and every flag are single quoted words.
    flags=
    for flag in "$@"; do flags="$flags $(quote "$flag")"; done
    wrap=$out/bin/$run
    [ ! -e "$wrap" ] || fail 73 "$wrap exists"
    {
        printf '%s\n' '#!/bin/sh'
        printf '%s\n' "F=$(quote "$FOUNDRY")"
        printf '%s\n' '[ "$1" = mcp ] || exec "$F" "$@"'
        printf '%s\n' "for a in \"\$@\"; do [ \"\$a\" = --semantic-profile ] && exec \"\$F\" \"\$@\" --development-isolation$flags; done"
        printf '%s\n' "exec \"\$F\" \"\$@\"$flags"
    } > "$wrap"
    chmod +x "$wrap"
    for c in rust bun; do
        clone=$out/runstores/$run-$c
        copy "$out/$c-$kind" "$clone"
        set -- run --questions "$QUESTIONS" --corpus $c --foundry "$wrap" --store "$clone" --out "$runs/$run"
        [ -z "$profile" ] || set -- "$@" --semantic-profile "$profile"
        python3 "$HARNESS" "$@"
        rm -rf "$clone"
    done
    ;;
compare)
    [ $# -eq 3 ] || usage
    name "$1"
    name "$2"
    case $3 in '' | 0 | *[!0-9]*) fail 64 "K must be a positive integer" ;; esac
    python3 "$HARNESS" compare --questions "$QUESTIONS" --base "$out/runs/$1" \
        --cand "$out/runs/$2" --profiles "$3"
    ;;
*) usage ;;
esac
