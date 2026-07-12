# LX Coreutils shell integration for fish
# Source this file or add to ~/.config/fish/config.fish:
#   source /path/to/shell-integration/lx.fish

# Ctrl+K: send current commandline to lxsh
function _lx_suggest
    set -l query (commandline)
    if test -z "$query"; return; end
    set -l result (printf '%s' "$query" | lxsh 2>/dev/null)
    if test -n "$result"
        commandline --replace "$result"
    end
end
bind \ck _lx_suggest

# Ctrl+E: explain current buffer before running.
# Echoes the command, clears the line, prints the explanation below.
function _lx_explain_buffer
    set -l cmd (commandline)
    if test -n "$cmd"
        commandline --replace ""
        commandline -f execute
        echo $cmd
        printf '%s' "$cmd" | lxexplain
    end
end
bind \ce _lx_explain_buffer

