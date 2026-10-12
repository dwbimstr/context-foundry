#!/bin/bash
# Bind the ecosystem work to the sandbox that holds the checkpoint (/workspace)
# through Claude Code Remote Control: the session runs on the sandbox, with its
# files, restored evidence and memory, and is driven from claude.ai/code or the
# Claude app.
#
#   scripts/cloud/sandbox.sh check   report whether Remote Control can run here
#   scripts/cloud/sandbox.sh start   start one Remote Control session in tmux
#   scripts/cloud/sandbox.sh status  show the running server
#   scripts/cloud/sandbox.sh stop    stop it (the session can be brought back
#                                    for about four hours with `start`)
#
# Extra arguments after `start` go to `claude remote-control`, for example
# `--permission-mode acceptEdits`.
set -u
REPO=${CF_REPO:-/workspace/context-foundry}
TMUX_SESSION=cf-remote-control
NAME=${CF_RC_NAME:-context-foundry sandbox}

pass() { echo "PASS $1: $2"; }
bad() { echo "FAIL $1: $2"; failed=1; }
info() { echo "INFO $1: $2"; }

check() {
  failed=0
  if command -v claude >/dev/null; then
    pass claude "$(command -v claude), $(claude --version 2>/dev/null | head -n 1)"
  else
    bad claude "not installed; install with: curl -fsSL https://claude.ai/install.sh | bash"
  fi
  if command -v claude >/dev/null; then
    local auth
    auth=$(claude auth status 2>/dev/null | python3 -c '
import json, sys
try:
    s = json.load(sys.stdin)
except Exception:
    print("unknown"); sys.exit()
print(s.get("loggedIn"), s.get("authMethod"), s.get("subscriptionType"), s.get("email"))
' 2>/dev/null)
    case "$auth" in
      "True claude.ai "*) pass auth "$auth" ;;
      "True "*) bad auth "$auth: Remote Control needs a claude.ai login (Pro/Max/Team/Enterprise), not an API key; run: claude auth login" ;;
      *) bad auth "${auth:-not logged in}: run claude auth login (prints a URL; paste the code back)" ;;
    esac
  fi
  for var in ANTHROPIC_BASE_URL CLAUDE_CODE_USE_BEDROCK CLAUDE_CODE_USE_VERTEX CLAUDE_CODE_USE_FOUNDRY \
      CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC DISABLE_GROWTHBOOK; do
    [ -n "${!var:-}" ] && bad "env $var" "set; Remote Control is unavailable while it is set"
  done
  [ -n "${ANTHROPIC_API_KEY:-}" ] && info "env ANTHROPIC_API_KEY" "set; unset it so the claude.ai login is used"
  for url in https://api.anthropic.com/ https://claude.ai/; do
    local code
    code=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 10 "$url" 2>/dev/null)
    if [ "$code" = 000 ]; then bad network "$url unreachable"; else pass network "$url HTTP $code"; fi
  done
  if command -v tmux >/dev/null; then pass tmux "$(tmux -V)"; else bad tmux "missing; the server must outlive this shell (apt-get install -y tmux)"; fi
  if pgrep -af 'claude.*remote-control' >/dev/null; then
    info running "$(pgrep -af 'claude.*remote-control' | head -n 3 | tr '\n' ' ')"
  fi

  if git -C "$REPO" rev-parse --git-dir >/dev/null 2>&1; then
    local dirty
    dirty=$(git -C "$REPO" status --porcelain --untracked-files=no | wc -l)
    pass repo "$REPO on $(git -C "$REPO" branch --show-current) at $(git -C "$REPO" rev-parse --short HEAD), $dirty modified"
  else
    bad repo "no repository at $REPO (set CF_REPO)"
  fi
  local v11=/workspace/.context-foundry-tools/validation-runs/realistic-delivery-v11
  [ -d "$v11" ] && pass evidence "$v11" || info evidence "$v11 missing"
  local quota mem
  quota=$(awk '$1 != "max" {printf "%.1f CPUs", $1 / $2}' /sys/fs/cgroup/cpu.max 2>/dev/null)
  mem=$(awk '$1 != "max" {printf "%.1f GiB limit", $1 / 1073741824}' /sys/fs/cgroup/memory.max 2>/dev/null)
  info host "$(uname -m), $(nproc) CPUs ${quota:+($quota quota)}, $(awk '/MemTotal/ {printf "%.1f GiB", $2 / 1048576}' /proc/meminfo) RAM ${mem:+($mem)}, $(awk '/SwapTotal/ {printf "%.1f GiB", $2 / 1048576}' /proc/meminfo) swap, $(df -h --output=avail /workspace 2>/dev/null | tail -n 1 | tr -d ' ') free"
  grep -qw avx512_bf16 /proc/cpuinfo && info cpu "avx512_bf16 yes" || info cpu "avx512_bf16 no"
  [ "$failed" = 0 ] && echo "READY: run scripts/cloud/sandbox.sh start" || echo "NOT READY: fix the FAIL lines"
  return "$failed"
}

start() {
  check >/dev/null || { check; return 1; }
  if tmux has-session -t "$TMUX_SESSION" 2>/dev/null; then
    echo "already running; attach with: tmux attach -t $TMUX_SESSION"
    return 0
  fi
  # One session (`--spawn session`): one writer for this tree, one heavy job at a time.
  local args=(remote-control --spawn session --name "$NAME" "$@")
  tmux new-session -d -s "$TMUX_SESSION" -c "$REPO" claude "${args[@]}"
  sleep 5
  tmux capture-pane -p -t "$TMUX_SESSION" | grep -v '^$' | tail -n 15
  cat <<EOF

The server runs in tmux session $TMUX_SESSION. The first start asks to trust
$REPO and to enable Remote Control: answer both in
  tmux attach -t $TMUX_SESSION     (detach with Ctrl-b d)
Then open claude.ai/code (or the Claude app) and pick "$NAME".
EOF
}

status() {
  if tmux has-session -t "$TMUX_SESSION" 2>/dev/null; then
    tmux capture-pane -p -t "$TMUX_SESSION" | grep -v '^$' | tail -n 15
  else
    echo "not running"
  fi
}

stop() {
  tmux send-keys -t "$TMUX_SESSION" C-c 2>/dev/null && sleep 3
  tmux kill-session -t "$TMUX_SESSION" 2>/dev/null
  echo "stopped"
}

cmd=${1:-check}
shift || true
case "$cmd" in
  check | start | status | stop) "$cmd" "$@" ;;
  *) echo "usage: $0 check|start|status|stop [remote-control flags]" >&2; exit 64 ;;
esac
