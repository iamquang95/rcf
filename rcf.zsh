rcf-append() {                                                                                                                                                 ✘ 130 
  rcf
  return cat /tmp/rcf.cmd
}
zle -N rcf-append
bindkey '^E' demo-append