#!/bin/bash

source ./.env

# run on windows host
 python.exe -m serial ${SERIAL_PORT} 115200