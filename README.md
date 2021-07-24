# CPU monitor daemon for OpenRGB

Requires [OpenRGB](https://openrgb.org) server running.

Monitors CPU load and changes desired LEDs according to the color circle, from
dark violet to red.

Written in Rust, fast, tiny and statically linked.

## Download binary

The pre-built binary is available for Linux x86\_64 on [releases
page](https://github.com/divi255/rgbmon/releases).

## Build from source

(requires Rust)
```
cargo build --release
```

## Usage

### Running

First run, for diagnostic:
```
rgbmon -v <options>
```

Run in background for production:
```
rgbmon -D <options>
```

Set the default color for CPU load < 20%:
```
rgbmon --default-color 20:99CCFF
```

By default, motherboard, DRAM, GPU, cooler and LED strip LEDs are used. The
tool doesn't allow customizing zones but the managed types can be selected.
E.g. manage MB and DRAM LEDS only:
```
rgbmon --device-types 0,1
```

### Events

Suspend LED management and turn them off.

Note: if used before system sleep, it's recommended to wait at least 0.5
seconds after the command, to make sure LEDs are turned off:
```
kill -USR1 $(cat /var/run/rgbmon.pid)
# sleep 0.5
```

Resume LED management, reload controllers from the server and forcibly
instantly set the color:
```
kill -USR1 $(cat /var/run/rgbmon.pid)
```

## Limitations

* Supports only device types, zones and individual LEDs are not supported
* Changes colors only, keeping modes untouched
* *src/lib.rs* contains a very basic client for OpenRGB SDK v2. If someone want
  to improve it to the fully functional client library crate - go on.
