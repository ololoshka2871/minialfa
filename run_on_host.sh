#!/usr/bin/sh

source ./.env

temp=$(mktemp)
file=$(wslpath -w ${temp})
winfile=$(basename ${temp})

cp $1 ${temp}

ssh ${SSH_TARTGET} "del tmp.*"
scp ${temp} ${SSH_TARTGET}:~
rm ${temp}
ssh ${SSH_TARTGET} "espflash flash -p ${SERIAL_PORT} --baud=921600 ${winfile}"