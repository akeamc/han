use core::fmt::{Debug, Display};
use core::str::FromStr;

use crate::{Error, Result};

#[derive(Debug, PartialEq, Eq)]
pub enum Line {
    Line1 = 0,
    Line2 = 1,
    Line3 = 2,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ActiveReactive {
    /// kW
    Active,
    /// kvar
    Reactive,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Direction {
    ToGrid,
    FromGrid,
}

use ActiveReactive::*;
use Direction::*;

#[derive(Debug, PartialEq, Eq)]
pub enum Object {
    DateTime(DateTime),
    /// Total energy (kWh or kvarh)
    TotalEnergy(ActiveReactive, Direction, Decimal<8, 3>),
    /// Power of all lines combined (kW or kvar)
    TotalPower(ActiveReactive, Direction, Decimal<4, 3>),
    Power(Line, ActiveReactive, Direction, Decimal<4, 3>),
    Voltage(Line, Decimal<3, 1>),
    Current(Line, Decimal<3, 1>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Obis(u8, u8, u8, u8, u8);

impl Display for Obis {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let Self(a, b, c, d, e) = self;
        write!(f, "{}-{}:{}.{}.{}", a, b, c, d, e)
    }
}

impl Obis {
    pub fn parse(s: &str) -> Option<Self> {
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
        Obis::parse(s).ok_or(Error::InvalidFormat)
    }
}

fn p<const I: u8, const F: u8>(s: &str) -> Result<Decimal<I, F>, Error> {
    let end = s.len().checked_sub(1).ok_or(Error::InvalidFormat)?;
    let inner = s.get(..end).ok_or(Error::InvalidFormat)?; // s has a trailing parenthesis
    let (scalar, _unit) = inner.split_once('*').ok_or(Error::InvalidFormat)?;

    scalar.parse()
}

/// Determine if the value specified is active or reactive, as well as the [`Direction`].
fn typ_dir(v: u8) -> Result<(ActiveReactive, Direction)> {
    match v {
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
            Obis(0, 0, 1, 0, 0) => Ok(Object::DateTime(DateTime::parse(body)?)),
            Obis(1, 0, c @ 1..=4, d @ 7..=8, 0) => {
                let (typ, dir) = typ_dir(c)?;
                match d {
                    7 => Ok(Object::TotalPower(typ, dir, p(body)?)),
                    8 => Ok(Object::TotalEnergy(typ, dir, p(body)?)),
                    _ => unreachable!(),
                }
            }
            Obis(1, 0, c @ 21..=24 | c @ 41..=44 | c @ 61..=64, 7, 0) => {
                let line = match c / 20 {
                    1 => Line::Line1,
                    2 => Line::Line2,
                    3 => Line::Line3,
                    _ => unreachable!(),
                };
                let (typ, dir) = typ_dir(c % 20)?;
                Ok(Object::Power(line, typ, dir, p(body)?))
            }
            Obis(1, 0, c @ 31..=32 | c @ 51..=52 | c @ 71..=72, 7, 0) => {
                let line = match c {
                    31..=32 => Line::Line1,
                    51..=52 => Line::Line2,
                    71..=72 => Line::Line3,
                    _ => unreachable!(),
                };

                match c % 10 {
                    1 => Ok(Object::Current(line, p(body)?)),
                    2 => Ok(Object::Voltage(line, p(body)?)),
                    _ => unreachable!(),
                }
            }
            _ => Err(Error::UnrecognizedReference),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct DateTime {
    pub year: u8,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,

    pub dst: bool,
}

impl DateTime {
    pub fn parse(s: &str) -> Result<Self> {
        let parsetwo = |i| {
            s.get(i..=(i + 1))
                .and_then(|s| s.parse().ok())
                .ok_or(Error::InvalidFormat)
        };

        Ok(Self {
            year: parsetwo(0)?,
            month: parsetwo(2)?,
            day: parsetwo(4)?,
            hour: parsetwo(6)?,
            minute: parsetwo(8)?,
            second: parsetwo(10)?,
            dst: match s.get(12..=12) {
                Some("W") => false,
                Some("S") => true,
                _ => return Err(Error::InvalidFormat),
            },
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Decimal<const I: u8, const F: u8>(u32); // up to 9 digits in base 10

impl<const I: u8, const F: u8> FromStr for Decimal<I, F> {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (i, f) = s.split_once('.').ok_or(Error::InvalidFormat)?;

        if i.len() != I.into() || f.len() != F.into() {
            return Err(Error::InvalidFormat);
        }
        let i: u32 = i.parse().map_err(|_| Error::InvalidFormat)?;
        let f: u32 = f.parse().map_err(|_| Error::InvalidFormat)?;

        Ok(Self(
            i.checked_mul(10u32.pow(F.into()))
                .ok_or(Error::InvalidFormat)?
                + f,
        ))
    }
}

impl<const I: u8, const F: u8> From<Decimal<I, F>> for f64 {
    fn from(n: Decimal<I, F>) -> Self {
        f64::from(n.0) / f64::from(10u32.pow(F.into()))
    }
}

impl<const I: u8, const F: u8> Decimal<I, F> {
    pub fn fraction(&self) -> u32 {
        self.0 % 10u32.pow(F.into())
    }

    pub fn integer(&self) -> u32 {
        (self.0 - self.fraction()) / 10u32.pow(F.into())
    }
}

impl<const I: u8, const F: u8> Debug for Decimal<I, F> {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let (i, f): (usize, usize) = (I.into(), F.into());
        fmt.debug_tuple("Decimal")
            .field(&format_args!(
                "{:0<i$}.{:0>f$}",
                self.integer(),
                self.fraction()
            ))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use crate::obis::{ActiveReactive, DateTime};

    use super::{Decimal, Direction, Object};

    #[test]
    fn datetime() {
        let date = "0-0:1.0.0(221022162844S)".parse::<Object>().unwrap();

        assert_eq!(
            date,
            Object::DateTime(DateTime {
                year: 22,
                month: 10,
                day: 22,
                hour: 16,
                minute: 28,
                second: 44,
                dst: true
            })
        );
    }

    #[test]
    fn reading() {
        let reading = "1-0:1.8.0(00006136.930*kWh)".parse::<Object>().unwrap();

        assert_eq!(
            reading,
            Object::TotalEnergy(
                ActiveReactive::Active,
                Direction::FromGrid,
                Decimal(6136930)
            )
        );
    }

    #[test]
    fn num() {
        assert_eq!(
            "0123.456".parse::<Decimal::<4, 3>>().unwrap(),
            Decimal(123456)
        );
        assert!("0.585".parse::<Decimal::<3, 3>>().is_err()); // wrong number of decimals

        assert_eq!(f64::from(Decimal::<3, 3>(146195)), 146.195);
    }
}
