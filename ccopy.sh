# Search Claude Code conversations and copy the matching message to clipboard.
# Two-pass: rg (fixed string) narrows files, jq decodes the matching message text.
# Usage: ccopy [-p project-or-session-id] <query>
ccopy() {
  local project="" q=""
  [[ "$1" == "-p" ]] && { project="$2"; shift 2; }
  q="$1"
  [[ -z "$q" ]] && { echo "Usage: ccopy [-p project-or-session-id] <query>" >&2; return 1; }

  _ccopy_clip() {
    if command -v pbcopy &>/dev/null; then pbcopy
    elif command -v xclip &>/dev/null; then xclip -selection clipboard
    elif command -v wl-copy &>/dev/null; then wl-copy
    else cat >/dev/null; echo "No clipboard command (pbcopy/xclip/wl-copy)" >&2; return 1
    fi
  }

  local base=~/.claude/projects
  local files=()

  if [[ -n "$project" ]]; then
    if [[ -d "$base/$project" ]]; then
      while IFS= read -r line; do files+=("$line"); done < <(
        rg -Fil --sortr modified --glob '*.jsonl' --glob '!**/subagents/**' -- "$q" "$base/$project" 2>/dev/null)
    else
      local -a dirs
      while IFS= read -r line; do dirs+=("$line"); done < <(
        find "$base" -maxdepth 1 -type d -name "*${project}*" 2>/dev/null)
      if (( ${#dirs[@]} )); then
        while IFS= read -r line; do files+=("$line"); done < <(
          rg -Fil --sortr modified --glob '*.jsonl' --glob '!**/subagents/**' -- "$q" "${dirs[@]}" 2>/dev/null)
      else
        local found
        found=$(find "$base" -type f -name "${project}.jsonl" ! -path '*/subagents/*' 2>/dev/null | head -1)
        [[ -n "$found" ]] || { echo "Not found: $project" >&2; return 1; }
        files=("$found")
      fi
    fi
  else
    while IFS= read -r line; do files+=("$line"); done < <(
      rg -Fil --sortr modified --glob '*.jsonl' --glob '!**/subagents/**' -- "$q" "$base" 2>/dev/null)
  fi

  local matches=()
  for f in "${files[@]}"; do
    [[ -f "$f" ]] || continue
    while IFS= read -r -d '' chunk; do
      [[ -n "$chunk" ]] && matches+=("$chunk")
    done < <(
      rg -FiN -- "$q" "$f" 2>/dev/null \
        | jq -r -j --arg q "$q" '
            .message.content[]?
            | select(.type=="text" and (.text | ascii_downcase | contains($q | ascii_downcase)))
            | .text, "\u0000"
          ' 2>/dev/null
    )
  done

  (( ${#matches[@]} )) || { echo "No match: $q" >&2; return 1; }

  local out
  if (( ${#matches[@]} == 1 )); then
    out="${matches[@]:0:1}"
  elif command -v fzf &>/dev/null; then
    out=$(printf '%s\0' "${matches[@]}" | fzf --read0 --height=50% --preview-window=wrap)
    [[ -z "$out" ]] && return 0
  else
    out="${matches[@]:0:1}"
  fi

  printf '%s\n' "$out" | _ccopy_clip
  printf '%s\n' "$out"
}