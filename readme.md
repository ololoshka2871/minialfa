# Minialfa

## Connections

### Display
SPI:
MOSI - GPIO23
MISO - NC - -
SCK - GPIO18
SS - GPIO5

GPIO:
RESET - GPIO22
DC - GPIO21

### Sensors
I2C:
SDA - GPIO26
SCL - GPIO25

### Encoder
GPIO:
BUTTON - GPIO19
V1 - GPIO16
V2 - GPIO4

### JTAG debugging
TDO - GPIO15
TMS - GPIO14
TCK - GPIO13
TDI - GPIO12
TRST - EN
VCC - +3.3V
GND - GND

### UART
RX - GPIO3
TX - GPIO1

### Klapan
GPIO:
OUT - GPIO12

### Thyracon sensor pinout
4 - +24V
5 - GND
10 - A
11 - B
12 - DGND

## Опыт эксплуатации
1. Необходимо снизить скорость откачки вакуума, иначе повтораяемость никакая
2. Датчик давления СКТБ нечуствителен на низких давлениях, поэтому используется восновном датчик `Thyracon`.
3. Целесообразно добавить возможность логорифмически менять пределы установки порога срабатывания `10 -> 1`, `0,9 -> 0,1`, `0,09 -> 0,01`
