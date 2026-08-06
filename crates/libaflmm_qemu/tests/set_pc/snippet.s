.text
.global target,target_new_pc

/* this is the function getting called by the harness */
target:
    mov $1, %rax
    ret

/* this is where we will set_pc after breaking on the 'target' label */
target_new_pc:
    mov $0, %rax
    ret
