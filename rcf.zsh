rcf-append() {
  rcf
  read mode cmd < /tmp/rcf.cmd
  RBUFFER=""
  LBUFFER="${cmd}"
  zle reset-prompt
  if [[ "${mode}" == "run" ]]; then zle accept-line; fi
}
zle -N rcf-append
bindkey '^R' rcf-append # Or whatever keybinding you want
