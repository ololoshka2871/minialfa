#!/bin/bash

source ./.env

# run on windows host
espflash.exe \
    flash \
    -p ${SERIAL_PORT} \
    --baud=921600 \
    --chip esp32 \
    $(wslpath -w "${1}")