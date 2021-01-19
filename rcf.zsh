rcf-append() {
  LBUFFER="${LBUFFER}$(cat /tmp/rcf.cmd)"
  local ret=$(cat /tmp/rcf.cmd)
  zle reset-prompt
  return $ret
}
zle -N rcf-append
bindkey '^R' rcf-append # Or whatever keybinding you want
