#!/bin/bash
source ./lib/common.sh
. "$DIR/helpers.sh"
readonly LIMIT=10
declare -r MAX=3
COUNT=0
export PATH_PREFIX=/usr

outer() {
    local x=1
    inner() {
        echo "$x"
    }
    inner
}

function with-dash {
    echo dash
}

function both() {
    echo both
}

if true; then
    NESTED=1
fi
