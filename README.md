# Minialfa

## Connections

### Display
* SPI:

| name | Pin |
|--- | --- |
| MOSI | GPIO23 |
| MISO | NC |
| SCK | GPIO18 |
| SS | GPIO5 |

* GPIO:

| name | Pin |
|--- | --- |
| RESET | GPIO22 |
| DC | GPIO21 |

### Sensors
* I2C

| name | Pin |
|--- | --- |
| SDA | GPIO26 |
| SCL | GPIO25 |

### Encoder
| name | Pin |
|--- | --- |
| BUTTON | GPIO19 |
| V1 | GPIO16 |
| V2 | GPIO4 |

### JTAG debugging
| name | Pin |
|--- | --- |
| TDO | GPIO15 | 
| TMS | GPIO14 | 
| TCK | GPIO13 | 
| TDI | GPIO12 | 
| TRST | EN | 
| VCC | +3.3V | 
| GND | GND | 

### UART
| name | Pin |
|--- | --- |
| RX | GPIO3 |
| TX | GPIO1 |

### Klapan
| name | Pin |
|--- | --- |
| OUT | GPIO12 |

### Thyracon sensor pinout
| name | Pin |
|--- | ---|
| 4 | +24V |
| 5 | GND |
| 10 | A |
| 11 | B |
| 12 | DGND |


## Опыт эксплуатации

1. Необходимо снизить скорость откачки вакуума, иначе повтораяемость никакая
2. Датчик давления СКТБ нечуствителен на низких давлениях, поэтому используется восновном датчик `Thyracon`.
3. Целесообразно добавить возможность логорифмически менять пределы установки порога срабатывания `10 -> 1`, `0,9 -> 0,1`, `0,09 -> 0,01`
4. Решено избавиться от датчика `Thyracon`, измерять давление по датчику `SCTB` до достижения порога, а затем еще N-секунд (настройки).

## Flash
Необходима утилита `espflash`
```shell
cargo install espflash
```

1. Вывезти ESP32 в режим программирования BOOT0 + Reset
2. Выполнить
```shell
espflash flash -p <SERIAL_PORT> --baud=921600 --chip esp32 target/xtensa-esp32-espidf/{debug,release}/minialfa
```