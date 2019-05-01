use std::f64;
use std::fmt::{self, Display};
use std::hash::{Hash, Hasher};
use std::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Neg, Not, Rem, Sub};

/// f64 wrapper with JS semantics
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct F64(f64);

impl F64 {
    pub const NAN: F64 = F64(f64::NAN);

    pub fn is_nan(self) -> bool {
        self.0.is_nan()
    }

    pub fn is_truthy(self) -> bool {
        self.0 != 0. && !self.0.is_nan()
    }

    pub fn into_inner(self) -> f64 {
        self.0
    }
}

impl From<f64> for F64 {
    fn from(x: f64) -> F64 {
        F64(x)
    }
}

impl Display for F64 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Hash for F64 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if self.0.is_nan() {
            // use one specific NaN representation
            state.write_u64(f64::NAN.to_bits());
        } else {
            state.write_u64(self.0.to_bits());
        }
    }
}

impl Neg for F64 {
    type Output = F64;
    fn neg(self) -> F64 {
        F64(-self.0)
    }
}

impl Not for F64 {
    type Output = F64;
    fn not(self) -> F64 {
        F64(!(self.0 as i32) as f64)
    }
}

impl Add for F64 {
    type Output = F64;
    fn add(self, rhs: F64) -> F64 {
        F64(self.0 + rhs.0)
    }
}

impl Sub for F64 {
    type Output = F64;
    fn sub(self, rhs: F64) -> F64 {
        F64(self.0 - rhs.0)
    }
}

impl Mul for F64 {
    type Output = F64;
    fn mul(self, rhs: F64) -> F64 {
        F64(self.0 * rhs.0)
    }
}

impl Div for F64 {
    type Output = F64;
    fn div(self, rhs: F64) -> F64 {
        F64(self.0 / rhs.0)
    }
}

impl Rem for F64 {
    type Output = F64;
    fn rem(self, rhs: F64) -> F64 {
        F64(self.0 % rhs.0)
    }
}

impl BitAnd for F64 {
    type Output = F64;
    fn bitand(self, rhs: F64) -> F64 {
        F64(((self.0 as i32) & (rhs.0 as i32)) as f64)
    }
}

impl BitOr for F64 {
    type Output = F64;
    fn bitor(self, rhs: F64) -> F64 {
        F64(((self.0 as i32) | (rhs.0 as i32)) as f64)
    }
}

impl BitXor for F64 {
    type Output = F64;
    fn bitxor(self, rhs: F64) -> F64 {
        F64(((self.0 as i32) ^ (rhs.0 as i32)) as f64)
    }
}

impl F64 {
    pub fn shl(self, rhs: F64) -> F64 {
        F64(((self.0 as i32) << rhs.0 as i32) as f64)
    }

    pub fn shr(self, rhs: F64) -> F64 {
        F64(((self.0 as i32) >> rhs.0 as i32) as f64)
    }

    pub fn shr_zero(self, rhs: F64) -> F64 {
        F64(((self.0 as i32 as u32) >> rhs.0 as i32) as f64)
    }

    pub fn powf(self, rhs: F64) -> F64 {
        F64(self.0.powf(rhs.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_ops() {
        let zero = F64::from(0.0);
        let one = F64::from(1.0);
        let two = F64::from(2.0);
        let three = F64::from(3.0);

        assert!(F64::NAN.is_nan());
        assert!(!zero.is_nan());

        assert!(!F64::NAN.is_truthy());
        assert!(!zero.is_truthy());
        assert!(one.is_truthy());

        assert_eq!(-one, F64::from(-1.));
        assert_eq!(!one, F64::from(-2.));

        assert_eq!(one + two, three);
        assert_eq!(three - one, two);
        assert_eq!(three / two, F64::from(3. / 2.));
        assert_eq!(three * two, F64::from(6.));
        assert_eq!(three % one, zero);

        assert_eq!(three & one, one);
        assert_eq!(two & one, zero);
        assert_eq!(three | one, three);
        assert_eq!(two | one, three);
        assert_eq!(two ^ one, three);
        assert_eq!(two ^ two, zero);

        assert_eq!(one.shl(one), two);
        assert_eq!(two.shr(one), one);
        assert_eq!((-two).shr(one), -one);
        assert_eq!(two.shr_zero(one), one);
        assert_eq!((-two).shr_zero(one), F64::from(2147483647.));
        assert_eq!(two.powf(two), F64::from(4.));
    }
}
