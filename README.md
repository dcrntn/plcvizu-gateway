# PLC visualization gateway
A simple gateway program to forward UDP packet from the PLC to Websocket clients. The base intent for this program, is to pass data from a PLC program to any HTML/JS based frontend. With this approach the default PLC visualization can be ditched and more custom visu can be written in HTML or basically any framework that supports websockets. 

## Note
The concept was based on the fact - that in some rare cases the built in CODESYS visualization isn't enough, or just a bit rigid. Also in no way this is an easier way of making visualizations for your PLC programs, since you have to know how to make webapps.   

The developement was done using a raspberry pi, with the CODESYS runtime. But in theory all PLCs - that support UDP sockets and let you run some custom programs on the OS - could be used.

## Example
An example HTML page and CODESYS project can be found [here](https://github.com/dcrntn/plcvizu-gateway-sample)

## Usage
1. Clone this repo, and build for your architecture.
2. If your arch is in the build folder already, just download the builded program and run it.

### Custom ports
```sh
# Run the program itself
./plcvizu-gateway
# > Websocket: 0.0.0.0:8081 | UDP read 127.0.0.1:34123 | UDP write: 127.0.0.1:34126

# Run with custom ports
./plcvizu-gateway 0.0.0.0:8888 127.0.0.1:39232 127.0.0.1:39234
# > Websocket: 0.0.0.0:8888 | UDP read 127.0.0.1:39232 | UDP write: 127.0.0.1:39234
```

