# LX Coreutils shell integration for zsh
# Source this file in ~/.zshrc:
#   source /path/to/shell-integration/lx.zsh

# Ctrl+K: send current buffer to lxsh
_lx_suggest() {
    local result
    result=$(printf '%s' "$BUFFER" | lxsh 2>/dev/null)
    if [[ -n "$result" ]]; then
        BUFFER="$result"
        CURSOR=${#BUFFER}
    fi
    zle redisplay
}
zle -N _lx_suggest
bindkey '^K' _lx_suggest

# Ctrl+E: explain current buffer before running.
# Echoes the command, clears the line, prints the explanation below.
_lx_explain_buffer() {
    local cmd="$BUFFER"
    if [[ -n "$cmd" ]]; then
        BUFFER=""
        zle accept-line
        echo "$cmd"
        printf '%s' "$cmd" | lxexplain
    fi
}
zle -N _lx_explain_buffer
bindkey '^E' _lx_explain_buffer

