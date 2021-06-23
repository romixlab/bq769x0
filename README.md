# bq769x0
TI BQ76920, BQ76930, BQ76940 HAL

Example usage, create instance, choose IC variant and cell count.
Variant is chosen at compile time, as hardware is generally not changed on the fly.
Cell count can be configured at runtime.

```rust
let mut bq76920 = BQ769x0::<X = bq769x0::BQ76920>::new(0x08, 4).unwrap();
```
`new()` will return None if cell count is invalid. Valid configurations is:
* BQ76920 - 3 to 5 cells
* BQ76930 - 6 to 10 cells
* BQ76940 - 9 to 15 cells

Configure thresholds and timeouts:
```rust
let bq76920_config = BQ769x0Config {
    shunt: MicroOhms(2000),
    scd_delay: SCDDelay::_400uS,
    scd_threshold: Amperes(100),
    ocd_delay: OCDDelay::_640ms,
    ocd_threshold: Amperes(50),
    uv_delay: UVDelay::_4s,
    uv_threshold: config::CELL_UV_THRESHOLD,
    ov_delay: OVDelay::_4s,
    ov_threshold: config::CELL_OV_THRESHOLD
};
let values = bq76920.init(i2c, &bq769x0_config).map_err(|e| Error::AfeError(e))?;
```
`values` will contain actual OCD & SCD range used as well as under voltage and over voltage thresholds as they depend on ADC calibration values stored in the device.

`init()` will return an error if:
* requested under or overvoltage thresholds are unobtainable
* requested short curcuit and overload current thresholds fall into different ranges (see datasheet, RSNS bit in PROTECT1 register)
* I2C communication fails (no or bad connection, bad IC, bad CRC or verify mismatch)

Disable DSG and CHG fets (be carefull with CHG=1 && DSG=0 or CHG=0 and DSG=1 configurations):
```rust
bq76920.discharge(i2c, false)?;
bq76920.charge(i2c, false)?;
```

Enable ADC and Coulomd counter for voltage and current measurements:
```rust
bq76920.enable_adc(i2c, true)?;
bq76920.coulomb_counter_mode(i2c, bq769x0::CoulombCounterMode::Continuous)?;

```

Show voltage, current and calculate power as well (fixed point math only):
```rust
let i = bq76920.current(i2c);
let v = bq76920.voltage(i2c);
let p: Result<i32, bq769x0::Error> = i.and_then(|i| {
    v.map(|v| i.0 * (v.0 as i32) / 1000)
});
match p {
    Ok(p) => {
        let intpart = p / 1000;
        let fractpart = p - intpart * 1000;
        writeln!(rtt, "V: {}, I:{}, Power: {}.{}W", v.unwrap(), i.unwrap(), intpart, fractpart.abs()).ok();
    }
    Err(e) => {
        writeln!(rtt, "Read error={:?}", e).ok();
    }
}
```

Show cell voltages:
```rust
match bq76920.cell_voltages(i2c) {
    Ok(cells) => {
        for (i, cell) in cells.iter().enumerate() {
            writeln!(rtt, "Cell {}: {}", i + 1, cell).ok();
        }
    }
    Err(e) => {
        writeln!(rtt, "Read error={:?}", e).ok();
    }
}
```

Show status and reset flags if needed:
```rust
let stat = bq76920.sys_stat(i2c);
writeln!(rtt, "{:?}", stat).ok();

let r = bq76920.sys_stat_reset(i2c, bq769x0::SysStat::SHORTCIRCUIT | bq769x0::SysStat::OVERCURRENT);
writeln!(rtt, "SCD|OCD clear: {:?}", r).ok();
```

Balancing is supported through `enable_balancing()` and `balancing_state()`, though is should be improved to not allow consecutive cells to balance.

Choose temperature source:
```rust
bq769x0.set_temperature_source(i2c, TemperatureSource::InternalDie);
```
Keep in mind that reading is not available right away, it will be scheculed internally and available after ?.

Read the temperature:
```rust
/// TODO: not finished, also mention and publish no hard float implementation of logarithm function.
```