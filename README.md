# Humidity and Temperature Data Server

Read data from a DHT22 sensor using a Raspberry Pi 1 or Zero and present it via http.

## Building

Raspberry Pi 1/Zero need a gcc for ARMv6, which you can get from [here](https://github.com/raspberrypi/tools/blob/master/arm-bcm2708/arm-rpi-4.9.3-linux-gnueabihf/bin/arm-linux-gnueabihf-gcc).
When building using the `build.sh` script, provide a variable `ARM6_GCC` containing the path to the gcc.

## Starting the server

Copy the executable to the Raspberry Pi.
Starting the server requires `sudo`, since a GPIO pin has to be accessed.

```sh
./humte <GPIO-pin> 0.0.0.0:<port>
```

The server reads the sensor data from `GPIO-pin` and listens for http requests on `port`.
