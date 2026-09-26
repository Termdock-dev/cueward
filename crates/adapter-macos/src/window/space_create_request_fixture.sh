#!/bin/sh
set -eu
directory=${CUEWARD_TEST_CREATE_REQUEST:?}
name=${CUEWARD_TEST_CREATE_NAME:?}
await_release() {
    count=0
    while [ ! -e "$directory/release-$1-$name" ]; do
        count=$((count + 1))
        [ "$count" -lt 800 ] || exit 9
        /bin/sleep 0.01
    done
}
if [ "${1-}" = '-fobjc-arc' ]; then
    : > "$directory/compiler-$name"
    await_release compile
    [ "$name" != 'compile-failure' ] || exit 7
    output=''
    while [ "$#" -gt 0 ]; do
        if [ "$1" = '-o' ]; then shift; output=$1; fi
        shift
    done
    /bin/cp "$directory/clang" "$output"
    /bin/chmod 700 "$output"
else
    /bin/cat > /dev/null
    : > "$directory/helper-$name"
    await_release readback
    [ "$name" != 'helper-failure' ] || exit 8
    printf '%s\n' '{"status":"sent_unverified","space_id":null,"display_id":null,"reason":"fixture","frontmost_pid_before":101,"frontmost_pid_after":101,"foreground_changed":false,"visible_spaces_before":[1],"visible_spaces_after":[1],"visible_spaces_changed":false}'
fi
