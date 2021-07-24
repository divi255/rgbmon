# CPU monitor daemon for OpenRGB

Requires [OpenRGB](https://openrgb.org) server.

Monitors CPU load and changes desired LEDs according to the color circle, from
dark violet to red.

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

Suspend LED management and turn them off:
```
kill -USR1 $(cat /var/run/rgbmon.pid)
```

Resume LED management and forcibly instantly set the color:
```
kill -USR1 $(cat /var/run/rgbmon.pid)
```

## Limitations

* Supports only device types, zones and individual LEDs are not supported
* Changes colors only, keeping modes untouched
* *src/lib.rs* contains very basic client for OpenRGB SDK v2. If someone want
  to improve it to the fully functional client library crate - go on.