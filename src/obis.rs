use core::fmt::{Debug, Display};
use core::str::FromStr;

use crate::{Error, Result};

/// One conductor in a three-phase system.
#[derive(Debug, PartialEq, Eq)]
pub enum Line {
    /// Line 1
    L1,
    /// Line 2
    L2,
    /// Line 3
    L3,
}

/// The type of power measured (*active* or *reactive*).
///
/// [Wikipedia](https://en.wikipedia.org/wiki/AC_power#Active,_reactive,_apparent,_and_complex_power_in_sinusoidal_steady-state)
#[derive(Debug, PartialEq, Eq)]
pub enum Power {
    /// Active power ([W](https://en.wikipedia.org/wiki/Watt)).
    Active,
    /// Reactive power ([VAr](https://en.wikipedia.org/wiki/Volt-ampere#Reactive)).
    Reactive,
}

/// Direction of the electricity flow.
#[derive(Debug, PartialEq, Eq)]
pub enum Direction {
    /// Energy received from the grid.
    FromGrid,
    /// Energy returned to the grid.
    ToGrid,
}

use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};
use Direction::*;
use Power::*;

/// A parsed line of the body of a [`Telegram`](crate::Telegram).
///
/// ```
/// use core::str::FromStr;
/// use han::{Object, Power, Direction};
///
/// // 1-0:3.8.0 specifies the received reactive energy.
/// let obj = "1-0:3.8.0(00000008.909*kvarh)".parse::<Object>()?;
/// assert_eq!(
///     obj,
///     Object::Energy(
///         Power::Reactive,
///         Direction::FromGrid,
///         8909, // VAr    
///     ),
/// );
/// # Ok::<(), han::Error>(())
/// ```
#[derive(Debug, PartialEq, Eq)]
pub enum Object {
    /// Timestamp with the correct timezone (CET/CEST[^dst]).
    ///
    /// [^dst]: According to the Swedish specification, only CET is ever used.
    ///     This library supports both, however.
    DateTime(OffsetDateTime),
    /// Energy received or returned across all [`Line`]s (Wh or VArh).
    Energy(Power, Direction, u32),
    /// Power of all lines combined (W or VAr).
    TotalPower(Power, Direction, u32),
    /// Power per [`Line`] (W or VAr).
    Power(Line, Power, Direction, u32),
    /// Phase voltage per [`Line`] measured in decivolts (dV, 0.1 V).
    Voltage(Line, u16),
    /// Phase current per [`Line`] (dA, 0.1 A).
    Current(Line, u16),
}

/// An *OBject Identifier System* identifier with the F group omitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Obis(pub u8, pub u8, pub u8, pub u8, pub u8);

impl Display for Obis {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let Self(a, b, c, d, e) = self;
        write!(f, "{}-{}:{}.{}.{}", a, b, c, d, e)
    }
}

impl Obis {
    fn from_str_opt(s: &str) -> Option<Self> {
        let (a, s) = s.split_once('-')?;
        let a = a.parse().ok()?;
        let (b, s) = s.split_once(':')?;
        let b = b.parse().ok()?;
        let mut iter = s.split('.').map(|part| part.parse().ok());

        Some(Self(a, b, iter.next()??, iter.next()??, iter.next()??))
    }
}

impl FromStr for Obis {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Obis::from_str_opt(s).ok_or(Error::InvalidFormat)
    }
}

/// Get the scalar and the unit from a value with a trailing parenthesis.
fn split_value(s: &str) -> Option<(&str, &str)> {
    let end = s.len().checked_sub(1)?; // s has a trailing parenthesis
    let inner = s.get(..end)?;
    inner.split_once('*')
}

fn parse_decimal<const F: u8>(s: &str) -> Option<u32> {
    let (decimal, _unit) = split_value(s)?;
    let (i, f) = decimal.rsplit_once('.')?;
    if f.len() != F.into() {
        return None;
    }
    let i: u32 = i.parse().ok()?;
    let f: u32 = f.parse().ok()?;

    i.checked_mul(10u32.pow(F.into()))?.checked_add(f)
}

fn parse_kilo(s: &str) -> Result<u32, Error> {
    parse_decimal::<3>(s).ok_or(Error::InvalidFormat)
}

fn parse_deci(s: &str) -> Result<u16, Error> {
    parse_decimal::<1>(s)
        .and_then(|v| v.try_into().ok())
        .ok_or(Error::InvalidFormat)
}

/// Determine if the power specified is active or reactive, as well as the [`Direction`].
fn pow_dir(a: u8) -> Result<(Power, Direction)> {
    match a {
        1 => Ok((Active, FromGrid)),
        2 => Ok((Active, ToGrid)),
        3 => Ok((Reactive, FromGrid)),
        4 => Ok((Reactive, ToGrid)),
        _ => Err(Error::InvalidFormat),
    }
}

impl FromStr for Object {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (obis, body) = s.split_once('(').ok_or(Error::InvalidFormat)?;
        let obis: Obis = obis.parse()?;

        match obis {
            Obis(0, 0, 1, 0, 0) => Ok(Object::DateTime(parse_datetime(body)?)),
            Obis(1, 0, c @ 1..=4, d @ 7..=8, 0) => {
                let (pow, dir) = pow_dir(c)?;
                match d {
                    7 => Ok(Object::TotalPower(pow, dir, parse_kilo(body)?)),
                    8 => Ok(Object::Energy(pow, dir, parse_kilo(body)?)),
                    _ => unreachable!(),
                }
            }
            Obis(1, 0, c @ 21..=24 | c @ 41..=44 | c @ 61..=64, 7, 0) => {
                let line = match c / 20 {
                    1 => Line::L1,
                    2 => Line::L2,
                    3 => Line::L3,
                    _ => unreachable!(),
                };
                let (pow, dir) = pow_dir(c % 20)?;
                Ok(Object::Power(line, pow, dir, parse_kilo(body)?))
            }
            Obis(1, 0, c @ 31..=32 | c @ 51..=52 | c @ 71..=72, 7, 0) => {
                let line = match c {
                    31..=32 => Line::L1,
                    51..=52 => Line::L2,
                    71..=72 => Line::L3,
                    _ => unreachable!(),
                };

                match c % 10 {
                    1 => Ok(Object::Current(line, parse_deci(body)?)),
                    2 => Ok(Object::Voltage(line, parse_deci(body)?)),
                    _ => unreachable!(),
                }
            }
            _ => Err(Error::UnrecognizedReference),
        }
    }
}

fn parse_datetime(s: &str) -> Result<OffsetDateTime> {
    let parsetwo = |i| {
        s.get(i..=(i + 1))
            .and_then(|s| s.parse::<u8>().ok())
            .ok_or(Error::InvalidFormat)
    };

    let year: i32 = i32::from(parsetwo(0)?) + 2000;
    let month = match s.get(2..4).ok_or(Error::InvalidFormat)? {
        "01" => Month::January,
        "02" => Month::February,
        "03" => Month::March,
        "04" => Month::April,
        "05" => Month::May,
        "06" => Month::June,
        "07" => Month::July,
        "08" => Month::August,
        "09" => Month::September,
        "10" => Month::October,
        "11" => Month::November,
        "12" => Month::December,
        _ => return Err(Error::InvalidFormat),
    };
    let day = parsetwo(4)?;
    let date = Date::from_calendar_date(year, month, day).map_err(|_| Error::InvalidFormat)?;
    let time = Time::from_hms(parsetwo(6)?, parsetwo(8)?, parsetwo(10)?)
        .map_err(|_| Error::InvalidFormat)?;

    let offset = match s.get(12..=12) {
        Some("W") => UtcOffset::from_hms(1, 0, 0).unwrap(),
        Some("S") => UtcOffset::from_hms(2, 0, 0).unwrap(),
        _ => return Err(Error::InvalidFormat),
    };

    Ok(PrimitiveDateTime::new(date, time).assume_offset(offset))
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use crate::Line;

    use super::{parse_datetime, Direction, Object, Power};

    #[test]
    fn datetime_obj() {
        assert_eq!(
            "0-0:1.0.0(221022162844W)".parse::<Object>().unwrap(),
            Object::DateTime(datetime!(2022-10-22 15:28:44 UTC))
        );
    }

    #[test]
    fn datetime_parsing() {
        assert!(parse_datetime("9999999999W").is_err());
        assert!(parse_datetime("aaaaaa").is_err());
        assert!(parse_datetime("220717231648").is_err()); // missing dst indicator

        // spec says dst shouldn't be used, but it never hurts to overdo timezones
        assert_eq!(
            parse_datetime("220717231648S").unwrap(),
            datetime!(2022-07-17 21:16:48 UTC)
        );
    }

    #[test]
    fn parse() {
        assert_eq!(
            "1-0:1.8.0(00006136.930*kWh)".parse::<Object>().unwrap(),
            Object::Energy(Power::Active, Direction::FromGrid, 6136930)
        );

        assert_eq!(
            "1-0:72.7.0(235.5*V)".parse::<Object>().unwrap(),
            Object::Voltage(Line::L3, 2355)
        );
    }
}
