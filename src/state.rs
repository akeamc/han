use crate::{Direction, Object, Power, Result, Telegram};

#[cfg(feature = "serde")]
use serde::Serialize;
use time::OffsetDateTime;

/// this name is terrible
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct ActRea {
    pub active: Option<f64>,
    pub reactive: Option<f64>,
}

impl ActRea {
    #[inline]
    fn insert(&mut self, pow: Power, v: f64) {
        match pow {
            Power::Active => self.active = Some(v),
            Power::Reactive => self.reactive = Some(v),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct Dir {
    pub to_grid: ActRea,
    pub from_grid: ActRea,
}

impl Dir {
    #[inline]
    fn insert(&mut self, dir: Direction, pow: Power, v: f64) {
        let dir = match dir {
            Direction::ToGrid => &mut self.to_grid,
            Direction::FromGrid => &mut self.from_grid,
        };

        dir.insert(pow, v)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]

pub struct Line {
    pub power: Dir,
    pub voltage: Option<f64>,
    pub current: Option<f64>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct State {
    #[cfg_attr(feature = "serde", serde(with = "time::serde::timestamp::option"))]
    pub datetime: Option<OffsetDateTime>,
    pub energy: Dir,
    pub power: Dir,
    pub lines: [Line; 3],
}

impl State {
    pub fn insert(&mut self, object: Object) {
        match object {
            Object::DateTime(datetime) => self.datetime = Some(datetime),
            Object::TotalEnergy(pow, dir, v) => self.energy.insert(dir, pow, v.into()),
            Object::TotalPower(pow, dir, v) => self.power.insert(dir, pow, v.into()),
            Object::Power(line, pow, dir, v) => {
                self.lines[line as usize].power.insert(dir, pow, v.into())
            }
            Object::Voltage(line, v) => {
                self.lines[line as usize].voltage = Some(v.into());
            }
            Object::Current(line, v) => {
                self.lines[line as usize].current = Some(v.into());
            }
        };
    }

    pub fn from_telegram(telegram: &Telegram) -> Result<Self> {
        let mut s = Self::default();

        for o in telegram.objects() {
            s.insert(o?);
        }

        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use crate::Reader;

    #[test]
    fn from_txt() {
        let bytes = include_bytes!("../test/ell.txt");
        let mut reader = Reader::new(bytes.iter().cloned());
        let state = reader
            .next()
            .unwrap()
            .to_telegram()
            .unwrap()
            .to_state()
            .unwrap();

        assert_eq!(state.power.from_grid.active, Some(0.806));
    }
}
