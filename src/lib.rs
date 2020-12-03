#![cfg_attr(not(std), no_std)]

use core::fmt;
use core::fmt::Formatter;
use embedded_hal;
use crc_any::CRCu8;

#[cfg(not(std))]
use cortex_m;

pub struct BQ769x0 {
    dev_address: u8, // 7bit address
    crc: CRCu8, // x8 + x2 + x + 1
    init_complete: bool,
    adc_gain: u16, // uV / LSB
    adc_offset: i8, // mV
    shunt: MicroOhms,
}

#[derive(Debug, Copy, Clone)]
pub enum Error {
    //#[cfg(crc)]
    CRCMismatch,
    I2CError,
    BufTooLarge,
    Uninitialized,
    VerifyError(u8),
    OCDSCDRangeMismatch,
    UVThresholdUnobtainable,
    OVThresholdUnobtainable,
}

// impl<E> From<E> for Error
//     where E: embedded_hal::blocking::i2c::WriteRead
// {
//     fn from(e: E) -> Self {
//         Error::I2CError
//     }
// }

pub struct Stat {
    bits: u8
}

impl Stat {
    pub fn cc_ready_is_set(&self) -> bool { self.bits & (1u8 << 7) != 0 }
    pub fn device_xready_is_set(&self) -> bool { self.bits & (1u8 << 5) != 0 }
    pub fn ovrd_alert_is_set(&self) -> bool { self.bits & (1u8 << 4) != 0 }
    pub fn undervoltage_is_set(&self) -> bool { self.bits & (1u8 << 3) != 0 }
    pub fn overvoltage_is_set(&self) -> bool { self.bits & (1u8 << 2) != 0 }
    pub fn scd_is_set(&self) -> bool { self.bits & (1u8 << 1) != 0 }
    pub fn ocd_is_set(&self) -> bool { self.bits & (1u8 << 0) != 0 }

    pub fn is_ok(&self) -> bool { self.bits & 0b0011_1111 == 0 }
}

impl fmt::Debug for Stat {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let _ = write!(f, "(");
        if self.cc_ready_is_set() {
            let _ = write!(f, "CC_READY, ");
        };
        if self.device_xready_is_set() {
            let _ = write!(f, "XREADY, ");
        };
        if self.ovrd_alert_is_set() {
            let _ = write!(f, "ALERT, ");
        };
        if self.undervoltage_is_set() {
            let _ = write!(f, "UV, ");
        };
        if self.overvoltage_is_set() {
            let _ = write!(f, "OV, ");
        };
        if self.scd_is_set() {
            let _ = write!(f, "SCD, ");
        };
        if self.ocd_is_set() {
            let _ = write!(f, "OCD, ");
        };
        write!(f, ")")
    }
}

pub enum SCDDelay {
    _70uS,
    _100uS,
    _200uS,
    _400uS
}

impl SCDDelay {
    pub fn bits(&self) -> u8 {
        match self {
            SCDDelay::_70uS =>  { 0x0 << 3 },
            SCDDelay::_100uS => { 0x1 << 3 },
            SCDDelay::_200uS => { 0x2 << 3 },
            SCDDelay::_400uS => { 0x3 << 3 },
        }
    }
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub struct Amperes(pub u32);

#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub struct MilliAmperes(pub i32);

#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub struct MicroOhms(pub u32);

#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub struct MilliVolts(pub u32);

impl fmt::Display for Amperes {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}A", self.0)
    }
}

impl fmt::Display for MilliAmperes {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}mA", self.0)
    }
}

impl fmt::Display for MilliVolts {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}mV", self.0)
    }
}

#[derive(Copy, Clone)]
pub enum SCDThreshold {
    // Lower range (RSNS = 0)
    _22mV   = 22,
    _33mV   = 33,
    _44mV   = 44,
    _56mV   = 56,
    _67mV   = 67,
    _78mV   = 78,
    _89mV   = 89,
    _100mV  = 100,
    // Upper range (RSNS = 1)
    //_44mV, same
    //_67mV, same
    //_89mV, same
    _111mV  = 111,
    _133mV  = 133,
    _155mV  = 155,
    _178mV  = 178,
    _200mV  = 200
}

#[derive(PartialEq, Clone, Debug)]
pub enum OCDSCDRange {
    Lower,
    Upper,
    Unknown
}

impl OCDSCDRange {
    pub fn bits(&self) -> u8 {
        match self {
            OCDSCDRange::Lower => { 0 << 7 },
            OCDSCDRange::Upper => { 1 << 7 },
            OCDSCDRange::Unknown => { unreachable!() },
        }
    }
}

impl SCDThreshold {
    pub fn range(&self) -> OCDSCDRange {
        use SCDThreshold::*;
        match self {
            _44mV | _67mV | _89mV => OCDSCDRange::Unknown,
            _111mV | _133mV | _155mV | _178mV | _200mV => OCDSCDRange::Upper,
            _ => OCDSCDRange::Lower
        }
    }

    pub fn bits(&self, range: OCDSCDRange) -> u8 {
        use OCDSCDRange::*;
        use SCDThreshold::*;
        match range {
            Lower => {
                match self {
                    _22mV => { 0x0 }, _33mV => { 0x1 }, _44mV => { 0x2 }, _56mV => { 0x3 },
                    _67mV => { 0x4 }, _78mV => { 0x5 }, _89mV => { 0x6 }, _100mV => { 0x7 },
                    _ => { 0x0 } // upper range values, should not happen, 0 to be safe
                }
            }
            Upper => {
                match self {
                    _44mV => { 0x0 },  _67mV => { 0x1 },  _89mV => { 0x2 },  _111mV => { 0x3 },
                    _133mV => { 0x4 }, _155mV => { 0x5 }, _178mV => { 0x6 }, _200mV => { 0x7 },
                    _ => { 0x0 }
                }
            }
            _ => { unreachable!() }
        }
    }

    pub fn from_mv(mv_threshold: u8) -> Self {
        use SCDThreshold::*;
        let thresholds = [_22mV, _33mV, _44mV, _56mV, _67mV, _78mV, _89mV,
            _100mV, _111mV, _133mV, _155mV, _178mV, _200mV];
        if mv_threshold < 22 {
            return _22mV;
        } else if mv_threshold > 200 {
            return _200mV;
        } else {
            for t in thresholds.iter() {
                if mv_threshold <= *t as u8 {
                    return *t;
                }
            }
        }
        unreachable!();
    }

    pub fn from_current(threshold: Amperes, shunt: MicroOhms) -> Self {
        let mv_threshold = threshold.0 * shunt.0 / 1000;
        Self::from_mv(mv_threshold as u8)
    }
}

pub enum OCDDelay {
    _8ms    = 0x0,
    _20ms   = 0x1,
    _40ms   = 0x2,
    _80ms   = 0x3,
    _160ms  = 0x4,
    _320ms  = 0x5,
    _640ms  = 0x6,
    _1280ms = 0x7
}

impl OCDDelay {
    pub fn bits(&self) -> u8 {
        match self {
            OCDDelay::_8ms =>    { 0x0 << 4 },
            OCDDelay::_20ms =>   { 0x1 << 4 },
            OCDDelay::_40ms =>   { 0x2 << 4 },
            OCDDelay::_80ms =>   { 0x3 << 4 },
            OCDDelay::_160ms =>  { 0x4 << 4 },
            OCDDelay::_320ms =>  { 0x5 << 4 },
            OCDDelay::_640ms =>  { 0x6 << 4 },
            OCDDelay::_1280ms => { 0x7 << 4 },
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum OCDThreshold {
    // Lower range (RSNS = 0)
    _8mV    = 8,
    _11mV   = 11,
    _14mV   = 14,
    _17mV   = 17,
    _19mV   = 19,
    _22mV   = 22,
    _25mV   = 25,
    _28mV   = 28,
    _31mV   = 31,
    _33mV   = 33,
    _36mV   = 36,
    _39mV   = 39,
    _42mV   = 42,
    _44mV   = 44,
    _47mV   = 47,
    _50mV   = 50,
    // Upper range (RSNS = 1)
    //_17mV, same
    //_22mV,
    //_28mV,
    //_33mV,
    //_39mV,
    //_44mV,
    //_50mV,
    _56mV   = 56,
    _61mV   = 61,
    _67mV   = 67,
    _72mV   = 72,
    _78mV   = 78,
    _83mV   = 83,
    _89mV   = 89,
    _94mV   = 94,
    _100mV  = 100,
}

impl OCDThreshold {
    pub fn range(&self) -> OCDSCDRange {
        use OCDThreshold::*;
        match self {
            _17mV | _22mV | _28mV | _33mV | _39mV | _44mV | _50mV => OCDSCDRange::Unknown,
            _56mV | _61mV | _67mV | _72mV | _78mV | _83mV | _89mV | _94mV | _100mV => OCDSCDRange::Upper,
            _ => OCDSCDRange::Lower
        }
    }

    pub fn bits(&self, range: OCDSCDRange) -> u8 {
        use OCDSCDRange::*;
        use OCDThreshold::*;
        match range {
            Lower => {
                match self {
                    _8mV =>  { 0x0 }, _11mV => { 0x1 }, _14mV => { 0x2 }, _17mV => { 0x3 },
                    _19mV => { 0x4 }, _22mV => { 0x5 }, _25mV => { 0x6 }, _28mV => { 0x7 },
                    _31mV => { 0x8 }, _33mV => { 0x9 }, _36mV => { 0xa }, _39mV => { 0xb },
                    _42mV => { 0xc }, _44mV => { 0xd }, _47mV => { 0xe }, _50mV => { 0xf },
                    _ => { 0x0 }
                }
            }
            Upper => {
                match self {
                    _17mV => { 0x0 }, _22mV => { 0x1 }, _28mV => { 0x2 }, _33mV => { 0x3 },
                    _39mV => { 0x4 }, _44mV => { 0x5 }, _50mV => { 0x6 }, _56mV => { 0x7 },
                    _61mV => { 0x8 }, _67mV => { 0x9 }, _72mV => { 0xa }, _78mV => { 0xb },
                    _83mV => { 0xc }, _89mV => { 0xd }, _94mV => { 0xe }, _100mV => { 0xf },
                    _ => { 0x0 }
                }
            }
            _ => { unreachable!() }
        }
    }

    pub fn from_mv(mv_threshold: u8) -> Self {
        use OCDThreshold::*;
        let thresholds = [_8mV , _11mV, _14mV, _17mV, _19mV, _22mV, _25mV, _28mV,
            _31mV, _33mV, _36mV, _39mV, _42mV, _44mV, _47mV, _50mV, _56mV, _61mV, _67mV, _72mV,
            _78mV, _83mV, _89mV, _94mV, _100mV];
        if mv_threshold < 8 {
            return _8mV;
        } else if mv_threshold > 100 {
            return _100mV;
        } else {
            for t in thresholds.iter() {
                if mv_threshold <= *t as u8 {
                    return *t;
                }
            }
        }
        unreachable!();
    }

    pub fn from_current(threshold: Amperes, shunt: MicroOhms) -> Self {
        let mv_threshold = threshold.0 * shunt.0 / 1000;
        Self::from_mv(mv_threshold as u8)
    }
}

pub enum UVDelay {
    _1s  = 0x0,
    _4s  = 0x1,
    _8s  = 0x2,
    _16s = 0x3
}

impl UVDelay {
    pub fn bits(&self) -> u8 {
        match self {
            UVDelay::_1s =>  { 0x0 << 6 },
            UVDelay::_4s =>  { 0x1 << 6 },
            UVDelay::_8s =>  { 0x2 << 6 },
            UVDelay::_16s => { 0x3 << 6 },
        }
    }
}

pub enum OVDelay {
    _1s  = 0x0,
    _4s  = 0x1,
    _8s  = 0x2,
    _16s = 0x3
}

impl OVDelay {
    pub fn bits(&self) -> u8 {
        match self {
            OVDelay::_1s =>  { 0x0 << 4 },
            OVDelay::_4s =>  { 0x1 << 4 },
            OVDelay::_8s =>  { 0x2 << 4 },
            OVDelay::_16s => { 0x3 << 4 },
        }
    }
}

pub struct Config {
    pub shunt: MicroOhms,
    pub scd_delay: SCDDelay,
    pub scd_threshold: Amperes,
    pub ocd_delay: OCDDelay,
    pub ocd_threshold: Amperes,
    pub uv_delay: UVDelay,
    pub uv_threshold: MilliVolts,
    pub ov_delay: OVDelay,
    pub ov_threshold: MilliVolts
}

#[derive(Debug)]
pub struct CalculatedValues {
    pub ocdscd_range_used: OCDSCDRange,
    pub scd_threshold: Amperes,
    pub ocd_threshold: Amperes,
    pub uv_threshold: MilliVolts,
    pub ov_threshold: MilliVolts
}

impl BQ769x0 {
    pub fn new(dev_address: u8) -> Self {
        BQ769x0 { dev_address, crc: CRCu8::crc8(), init_complete: false, adc_gain: 0, adc_offset: 0, shunt: MicroOhms(0) }
    }

    pub fn adc_gain(&self) -> u16 {
        self.adc_gain
    }

    pub fn adc_offset(&self) -> i8 {
        self.adc_offset
    }

    // #[cfg(not(crc))]
    // pub fn read_raw<I2C>(&mut self, i2c: &mut I2C, reg_address: u8, data: &mut [u8]) -> Result<(), Error>
    //     where I2C: embedded_hal::blocking::i2c::Write + embedded_hal::blocking::i2c::WriteRead
    // {
    //     #[cfg(no_std)] {
    //         cortex_m::asm::delay(10000);
    //     }
    //
    //     match i2c.write_read(self.dev_address, &[reg_address], data) {
    //         Ok(_) => { Ok(()) },
    //         Err(_) => { Err(Error::I2CError) },
    //     }
    // }

    //#[cfg(crc)]
    pub fn read_raw<I2C>(&mut self, i2c: &mut I2C, reg_address: u8, data: &mut [u8]) -> Result<(), Error>
        where I2C: embedded_hal::blocking::i2c::Write + embedded_hal::blocking::i2c::WriteRead
    {
        //#[cfg(no_std)] {
            cortex_m::asm::delay(10000);
        //}

        if data.len() > 10 {
            return Err(Error::BufTooLarge);
        } else if data.len() == 0 {
            return Ok(());
        }
        let mut buf = [0u8; 10*2]; // byte,crc,byte,crc,...
        let r = i2c.write_read(self.dev_address, &[reg_address], &mut buf[0..data.len()*2]);
        self.crc.reset();
        self.crc.digest(&[(self.dev_address << 1) | 0b0000_0001, buf[0]]);
        if self.crc.get_crc() != buf[1] {
            return Err(Error::CRCMismatch);
        }
        if data.len() > 1 {
            for i in (3..data.len()*2).step_by(2) {
                self.crc.reset();
                self.crc.digest(&[buf[i - 1]]);
                if self.crc.get_crc() != buf[i] {
                    return Err(Error::CRCMismatch);
                }
            }
        }
        return if r.is_ok() {
            for (i, b) in data.iter_mut().enumerate() {
                *b = buf[i * 2];
            }
            Ok(())
        } else {
            Err(Error::I2CError)
        }
    }

    // #[cfg(not(crc))]
    // pub fn write_raw<I2C>(&mut self, i2c: &mut I2C, reg_address: u8, data: &[u8]) -> Result<(), Error>
    //     where I2C: embedded_hal::blocking::i2c::Write + embedded_hal::blocking::i2c::WriteRead
    // {
    //     #[cfg(no_std)] {
    //         cortex_m::asm::delay(10000);
    //     }
    //
    //     if data.len() > 8 {
    //         return Err(Error::BufTooLarge);
    //     } else if data.len() == 0 {
    //         return Ok(());
    //     }
    //     let mut buf = [0u8; 8+1]; // reg,byte,byte,...
    //     buf[0] = reg_address;
    //     for (i, b) in data.iter().enumerate() {
    //         buf[i + 1] = *b;
    //     }
    //
    //     match i2c.write(self.dev_address, &buf[0..data.len()+1]) {
    //         Ok(_) => { Ok(()) },
    //         Err(_) => { Err(Error::I2CError) },
    //     }
    // }

    //#[cfg(crc)]
    pub fn write_raw<I2C>(&mut self, i2c: &mut I2C, reg_address: u8, data: &[u8]) -> Result<(), Error>
        where I2C: embedded_hal::blocking::i2c::Write + embedded_hal::blocking::i2c::WriteRead
    {
        //#[cfg(no_std)] {
            cortex_m::asm::delay(10000);
        //}

        if data.len() > 8 {
            return Err(Error::BufTooLarge);
        } else if data.len() == 0 {
            return Ok(());
        }
        let mut buf = [0u8; 8*2+1]; // reg,byte,crc,byte,crc,...
        buf[0] = reg_address;
        for (i, b) in data.iter().enumerate() {
            buf[i * 2 + 1] = *b;
        }
        self.crc.reset();
        self.crc.digest(&[(self.dev_address << 1), reg_address, data[0]]);
        buf[2] = self.crc.get_crc();
        for i in (4..data.len()*2+1).step_by(2) {
            self.crc.reset();
            self.crc.digest(&[ buf[i-1] ]);
            buf[i] = self.crc.get_crc();
        }
        i2c.write(self.dev_address, &buf[0..data.len()*2+1]).map_err(|_| Error::I2CError)?;

        Ok(())
    }

    fn read_adc_characteristics<I2C>(&mut self, i2c: &mut I2C) -> Result<(), Error>
        where I2C: embedded_hal::blocking::i2c::Write + embedded_hal::blocking::i2c::WriteRead
    {
        let mut gain1_offset = [0u8; 2];
        let mut gain2 = [0u8; 1];
        self.read_raw(i2c, 0x50, &mut gain1_offset)?;
        self.read_raw(i2c, 0x59, &mut gain2)?;
        self.adc_gain = 365 + ( ((gain1_offset[0] << 1) & 0b0001_1000) | (gain2[0] >> 5) ) as u16;
        self.adc_offset = gain1_offset[1] as i8;

        Ok(())
    }

    pub fn is_initialized(&self) -> bool {
        self.init_complete
    }

    pub fn cell_voltages<I2C>(&mut self, i2c: &mut I2C) -> Result<[u16; 5], Error>
        where I2C: embedded_hal::blocking::i2c::Write + embedded_hal::blocking::i2c::WriteRead
    {
        if !self.is_initialized() {
            return Err(Error::Uninitialized);
        }
        let mut buf = [0u8; 10];
        self.read_raw(i2c, 0x0c, &mut buf)?;
        let mut cells = [0u16; 5];
        for (i, cell) in cells.iter_mut().enumerate() {
            let adc_reading = ((buf[i * 2] as u16) << 8) | buf[i * 2 + 1] as u16;
            *cell = self.adc_reading_to_mv(adc_reading).0 as u16;
        }

        Ok(cells)
    }

    pub fn current<I2C>(&mut self, i2c: &mut I2C) -> Result<MilliAmperes, Error>
        where I2C: embedded_hal::blocking::i2c::Write + embedded_hal::blocking::i2c::WriteRead
    {
        // let mut sys_ctrl2 = [0u8; 1];
        // self.read_raw(i2c, 0x05, &mut sys_ctrl2)?;
        // sys_ctrl2[0] = sys_ctrl2[0] | 0b0010_0000;
        // self.write_raw(i2c, 0x05, &sys_ctrl2)?;
        // delay(8_000_000);
        let mut cc = [0u8; 2];
        self.read_raw(i2c, 0x32, &mut cc)?;
        let cc = i16::from_be_bytes(cc);
        let vshunt = cc as i32 * 8440; // nV
        let current = vshunt / self.shunt.0 as i32;
        Ok(MilliAmperes(current))
    }

    pub fn voltage<I2C>(&mut self, i2c: &mut I2C) -> Result<MilliVolts, Error>
        where I2C: embedded_hal::blocking::i2c::Write + embedded_hal::blocking::i2c::WriteRead
    {
        // let mut sys_ctrl2 = [0u8; 1];
        // self.read_raw(i2c, 0x05, &mut sys_ctrl2)?;
        // sys_ctrl2[0] = sys_ctrl2[0] | 0b0010_0000;
        // self.write_raw(i2c, 0x05, &sys_ctrl2)?;
        // delay(8_000_000);
        let mut vv = [0u8; 2];
        self.read_raw(i2c, 0x2a, &mut vv)?;
        let vv = u16::from_be_bytes(vv);
        let voltage = 4 * (self.adc_gain as i32) * (vv as i32) + 5 * (self.adc_offset as i32) * 1000;
        Ok(MilliVolts((voltage / 1000) as u32))
    }

    pub fn sys_stat<I2C>(&mut self, i2c: &mut I2C) -> Result<Stat, Error>
        where I2C: embedded_hal::blocking::i2c::Write + embedded_hal::blocking::i2c::WriteRead
    {
        let mut data = [0u8; 1];
        self.read_raw(i2c, 0x00, &mut data)?;
        Ok(Stat{ bits: data[0] })
    }

    pub fn sys_stat_reset<I2C>(&mut self, i2c: &mut I2C) -> Result<(), Error>
        where I2C: embedded_hal::blocking::i2c::Write + embedded_hal::blocking::i2c::WriteRead
    {
        self.write_raw(i2c, 0x00, &[0xff])?;
        let mut verify = [0u8; 1];
        self.read_raw(i2c, 0x00, &mut verify)?;
        if verify[0] != 0 {
            return Err(Error::VerifyError(0x00));
        }
        Ok(())
    }

    pub fn discharge<I2C>(&mut self, i2c: &mut I2C, enable: bool) -> Result<(), Error>
        where I2C: embedded_hal::blocking::i2c::Write + embedded_hal::blocking::i2c::WriteRead
    {
        let mut sys_ctrl2 = [0u8; 1];
        self.read_raw(i2c, 0x05, &mut sys_ctrl2)?;
        let already_enabled = sys_ctrl2[0] & 0b0000_0010 != 0;
        if enable == already_enabled {
            return Ok(())
        }
        if enable {
            sys_ctrl2[0] = sys_ctrl2[0] | 0b0000_0010;
        } else {
            sys_ctrl2[0] = sys_ctrl2[0] & !0b0000_0010;
        }
        self.write_raw(i2c, 0x05, &sys_ctrl2)?;
        let mut verify = [0u8; 1];
        self.read_raw(i2c, 0x05, &mut verify)?;
        if verify[0] != sys_ctrl2[0] {
            return Err(Error::VerifyError(0x05));
        }
        Ok(())
    }

    pub fn charge<I2C>(&mut self, i2c: &mut I2C, enable: bool) -> Result<(), Error>
        where I2C: embedded_hal::blocking::i2c::Write + embedded_hal::blocking::i2c::WriteRead
    {
        let mut sys_ctrl2 = [0u8; 1];
        self.read_raw(i2c, 0x05, &mut sys_ctrl2)?;
        let already_enabled = sys_ctrl2[0] & 0b0000_0001 != 0;
        if enable == already_enabled {
            return Ok(())
        }
        if enable {
            sys_ctrl2[0] = sys_ctrl2[0] | 0b0000_0001;
        } else {
            sys_ctrl2[0] = sys_ctrl2[0] & !0b0000_0001;
        }
        self.write_raw(i2c, 0x05, &sys_ctrl2)?;
        let mut verify = [0u8; 1];
        self.read_raw(i2c, 0x05, &mut verify)?;
        if verify[0] != sys_ctrl2[0] {
            return Err(Error::VerifyError(0x05));
        }
        Ok(())
    }

    pub fn is_charging<I2C>(&mut self, i2c: &mut I2C) -> Result<bool, Error>
        where I2C: embedded_hal::blocking::i2c::Write + embedded_hal::blocking::i2c::WriteRead
    {
        let mut sys_ctrl2 = [0u8; 1];
        self.read_raw(i2c, 0x05, &mut sys_ctrl2)?;
        Ok(sys_ctrl2[0] & 0b0000_0001 != 0)
    }

    pub fn ship_enter<I2C>(&mut self, i2c: &mut I2C) -> Result<(), Error>
        where I2C: embedded_hal::blocking::i2c::Write + embedded_hal::blocking::i2c::WriteRead
    {
        self.write_raw(i2c, 0x04, &[0b0000_0000])?;
        self.write_raw(i2c, 0x04, &[0b0000_0001])?;
        self.write_raw(i2c, 0x04, &[0b0000_0010])?;
        Ok(())
    }

    fn adc_reading_to_mv(&self, adc_reading: u16) -> MilliVolts {
        let adc_reading = adc_reading as i32;
        let uv = adc_reading * self.adc_gain as i32 + self.adc_offset as i32 * 1000;
        MilliVolts((uv / 1000) as u32)
    }

    fn ov_voltage_range(&self) -> (MilliVolts, MilliVolts) {
        let min_adc_reading = 0b10_0000_0000_1000;
        let max_adc_reading = 0b10_1111_1111_1000;
        (self.adc_reading_to_mv(min_adc_reading), self.adc_reading_to_mv(max_adc_reading))
    }

    fn uv_voltage_range(&self) -> (MilliVolts, MilliVolts) {
        let min_adc_reading = 0b01_0000_0000_0000;
        let max_adc_reading = 0b01_1111_1111_0000;
        (self.adc_reading_to_mv(min_adc_reading), self.adc_reading_to_mv(max_adc_reading))
    }

    pub fn init<I2C>(&mut self, i2c: &mut I2C, config: &Config) -> Result<CalculatedValues, Error>
        where I2C: embedded_hal::blocking::i2c::Write + embedded_hal::blocking::i2c::WriteRead
    {
        self.read_adc_characteristics(i2c)?;

        let scd_threshold = SCDThreshold::from_current(config.scd_threshold, config.shunt);
        let ocd_threshold = OCDThreshold::from_current(config.ocd_threshold, config.shunt);
        let scd_range = scd_threshold.range();
        let ocd_range = ocd_threshold.range();
        if (scd_range == OCDSCDRange::Lower && ocd_range == OCDSCDRange::Upper) ||
            (scd_range == OCDSCDRange::Upper && ocd_range == OCDSCDRange::Lower) {
            return Err(Error::OCDSCDRangeMismatch);
        }
        let range_to_use = if scd_range == OCDSCDRange::Unknown {
            if ocd_range == OCDSCDRange::Unknown {
                OCDSCDRange::Lower
            } else {
                ocd_range
            }
        } else if ocd_range == OCDSCDRange::Unknown {
            if scd_range == OCDSCDRange::Unknown {
                OCDSCDRange::Lower
            } else {
                scd_range
            }
        } else {
            ocd_range // both ranges are equal
        };
        let scd_bits = scd_threshold.bits(range_to_use.clone());
        let ocd_bits = ocd_threshold.bits(range_to_use.clone());

        let mut regs = [0u8; 6];
        regs[0] = range_to_use.bits() | config.scd_delay.bits() | scd_bits; // PROTECT1 (0x06)
        regs[1] = config.ocd_delay.bits() | ocd_bits; // PROTECT2 (0x07)
        regs[2] = config.uv_delay.bits() | config.ov_delay.bits(); // PROTECT3 (0x08)

        let ov_limits = self.ov_voltage_range();
        if !(config.ov_threshold >= ov_limits.0 && config.ov_threshold <= ov_limits.1) {
            return Err(Error::OVThresholdUnobtainable);
        }
        let ov_trip_full = ((config.ov_threshold.0 as i32 - self.adc_offset as i32) * 1000) / self.adc_gain as i32; // ADC value * 1000
        let ov_bits = (((ov_trip_full as u16) >> 4) & 0xff) as u8;

        let uv_limits = self.uv_voltage_range();
        if !(config.uv_threshold >= uv_limits.0 && config.uv_threshold <= uv_limits.1) {
            return Err(Error::UVThresholdUnobtainable);
        }
        let uv_trip_full = ((config.uv_threshold.0 as i32 - self.adc_offset as i32) * 1000) / self.adc_gain as i32; // ADC value * 1000
        let uv_bits = (((uv_trip_full as u16) >> 4) & 0xff) as u8;

        regs[3] = ov_bits; // (0x09)
        regs[4] = uv_bits; // (0xA)
        regs[5] = 0x19; // (0xB)

        self.write_raw(i2c, 0x06, &regs)?;
        self.shunt = config.shunt;
        self.init_complete = true;

        let mut sysctrl2 = [0u8; 1];
        self.read_raw(i2c, 0x05, &mut sysctrl2)?;
        sysctrl2[0] = sysctrl2[0] | 0b0100_0000; // !!CC_EN!!
        self.write_raw(i2c, 0x05, &sysctrl2)?;

        Ok(CalculatedValues{
            ocdscd_range_used: range_to_use,
            scd_threshold: Amperes(((scd_threshold as u32) * 1000) / config.shunt.0),
            ocd_threshold: Amperes(((ocd_threshold as u32) * 1000) / config.shunt.0),
            uv_threshold: self.adc_reading_to_mv(0b01_0000_0000_0000 | ((uv_bits as u16) << 4)),
            ov_threshold: self.adc_reading_to_mv(0b10_0000_0000_1000 | ((ov_bits as u16) << 4))
        })
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    struct DummyI2C {
        pub regs: [u8; 255],
    }

    impl DummyI2C {
        pub fn new() -> Self {
            let mut regs = [0u8; 255];
            regs[0x50] = 0x15;
            regs[0x51] = 0x2b;
            regs[0x59] = 0xa3;
            DummyI2C { regs }
        }
    }

    impl embedded_hal::blocking::i2c::Write for DummyI2C {
        type Error = ();

        fn write(&mut self, addr: u8, bytes: &[u8]) -> Result<(), Self::Error> {
            std::println!("-----------");
            std::println!("write: {:#04x}", addr);
            let base_reg_addr = bytes[0] as usize;
            for (i, b) in bytes.iter().skip(1).enumerate() {
                let reg_addr = base_reg_addr + i;
                self.regs[reg_addr] = *b;
                std::println!("{}/{:#04x}\t<= {:#04x}={:#010b}", reg_addr, reg_addr, *b, *b);
            }

            Ok(())
        }
    }

    impl embedded_hal::blocking::i2c::WriteRead for DummyI2C {
        type Error = ();

        fn write_read(&mut self, address: u8, bytes: &[u8], buffer: &mut [u8]) -> Result<(), Self::Error> {
            std::println!("----------------");
            std::println!("write_read: {:#04x}", address);
            let base_reg_addr = bytes[0] as usize;
            for (i, b) in buffer.iter_mut().enumerate() {
                let reg_addr = base_reg_addr + i;
                let reg_value = self.regs[reg_addr];
                *b = reg_value;
                std::println!("{}/{:#04x}\t== {:#04x}={:#010b}", reg_addr, reg_addr, reg_value, reg_value);
            }

            Ok(())
        }
    }

    #[test]
    fn it_works() {
        use crate::*;

        let mut i2c = DummyI2C::new();
        let mut bq769x0 = BQ769x0::new(0x08);
        let config = Config {
            shunt: MicroOhms(667),
            scd_delay: SCDDelay::_400uS,
            scd_threshold: Amperes(200),
            ocd_delay: OCDDelay::_1280ms,
            ocd_threshold: Amperes(100),
            uv_delay: UVDelay::_4s,
            uv_threshold: MilliVolts(2000),
            ov_delay: OVDelay::_4s,
            ov_threshold: MilliVolts(4175)
        };
        match bq769x0.init(&mut i2c, &config) {
            Ok(actual) => {
                std::println!("bq769x0 init ok");
                std::println!("adc gain:{}uV/LSB offset:{}mV", bq769x0.adc_gain(), bq769x0.adc_offset());
                std::println!("SCD: {}, OCD: {}, range: {:?}", actual.scd_threshold, actual.ocd_threshold, actual.ocdscd_range_used);
                std::println!("UV: {}, OV: {}", actual.uv_threshold, actual.ov_threshold);
            }
            Err(e) => {
                std::println!("bq769x0 init err: {:?}", e);
            }
        }
    }
}
