# LX Coreutils shell integration for bash
# Source this file in ~/.bashrc:
#   source /path/to/shell-integration/lx.bash

# Ctrl+K: send current readline buffer to lxsh
_lx_suggest() {
    local query="$READLINE_LINE"
    if [[ -z "$query" ]]; then return; fi
    local result
    result=$(printf '%s' "$query" | lxsh 2>/dev/null)
    if [[ -n "$result" ]]; then
        READLINE_LINE="$result"
        READLINE_POINT=${#result}
    fi
}
bind -x '"\C-k": _lx_suggest'

# Ctrl+E: explain current buffer before running.
# Echoes the command, clears the line, prints the explanation below.
_lx_explain_buffer() {
    local cmd="$READLINE_LINE"
    if [[ -n "$cmd" ]]; then
        READLINE_LINE=""
        echo "$cmd"
        printf '%s' "$cmd" | lxexplain
    fi
}
bind -x '"\C-e": _lx_explain_buffer'

