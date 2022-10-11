target remote :2331
set remotetimeout 10000
monitor reset halt
shell cargo espflash save-image --chip ESP32 target/firmware.bin
monitor program_esp target/firmware.bin 0x10000 verify
quit
