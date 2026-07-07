// Several constructors/ops are consumed starting in the accumulation-AD phase;
// allow the transient dead-code window while the assembler is migrated.
#![allow(dead_code)]

//! Fixed-size forward-mode automatic differentiation.
//!
//! `Ad<N>` is a dual number carrying a value plus `N` partial derivatives with
//! respect to a local block of primary variables. It is the building block for
//! the FIM residual/Jacobian assembly: every residual primitive is written once
//! over a scalar type, evaluated with `f64` for the plain residual and with
//! `Ad<N>` to obtain an exact, self-consistent Jacobian.
//!
//! This mirrors OPM Flow's `DenseAd::Evaluation` local-AD approach, specialized
//! to a small fixed `N` (the per-cell block size, plus a couple of extra well
//! directions) so there is no heap allocation and it stays wasm-friendly.

use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// Forward-mode dual number with `N` derivative slots.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Ad<const N: usize> {
    value: f64,
    deriv: [f64; N],
}

impl<const N: usize> Ad<N> {
    /// A constant (all partial derivatives zero).
    pub(crate) fn constant(value: f64) -> Self {
        Self {
            value,
            deriv: [0.0; N],
        }
    }

    /// An independent variable: `value` with a unit derivative in slot `slot`.
    pub(crate) fn variable(value: f64, slot: usize) -> Self {
        let mut deriv = [0.0; N];
        deriv[slot] = 1.0;
        Self { value, deriv }
    }

    /// Construct from an explicit value and derivative array.
    pub(crate) fn seeded(value: f64, deriv: [f64; N]) -> Self {
        Self { value, deriv }
    }

    pub(crate) fn value(&self) -> f64 {
        self.value
    }

    pub(crate) fn deriv(&self) -> &[f64; N] {
        &self.deriv
    }

    /// Derivative with respect to slot `slot`.
    pub(crate) fn d(&self, slot: usize) -> f64 {
        self.deriv[slot]
    }

    pub(crate) fn exp(self) -> Self {
        let value = self.value.exp();
        Self {
            value,
            deriv: self.deriv.map(|g| g * value),
        }
    }

    pub(crate) fn ln(self) -> Self {
        let inv = 1.0 / self.value;
        Self {
            value: self.value.ln(),
            deriv: self.deriv.map(|g| g * inv),
        }
    }

    pub(crate) fn sqrt(self) -> Self {
        let value = self.value.sqrt();
        let coef = 0.5 / value;
        Self {
            value,
            deriv: self.deriv.map(|g| g * coef),
        }
    }

    pub(crate) fn recip(self) -> Self {
        let value = 1.0 / self.value;
        let coef = -value * value;
        Self {
            value,
            deriv: self.deriv.map(|g| g * coef),
        }
    }

    /// `self^p` for a constant exponent `p`.
    pub(crate) fn powf(self, p: f64) -> Self {
        let value = self.value.powf(p);
        let coef = p * self.value.powf(p - 1.0);
        Self {
            value,
            deriv: self.deriv.map(|g| g * coef),
        }
    }

    pub(crate) fn powi(self, n: i32) -> Self {
        let value = self.value.powi(n);
        let coef = (n as f64) * self.value.powi(n - 1);
        Self {
            value,
            deriv: self.deriv.map(|g| g * coef),
        }
    }

    /// Branch-selecting maximum: the result carries the value and derivatives of
    /// whichever operand is larger (matching a clamp/upwind switch that is frozen
    /// within a Newton iteration). Non-smooth exactly at the crossover point.
    pub(crate) fn max(self, other: Self) -> Self {
        if self.value >= other.value {
            self
        } else {
            other
        }
    }

    pub(crate) fn min(self, other: Self) -> Self {
        if self.value <= other.value {
            self
        } else {
            other
        }
    }

    /// Branch-selecting max against a constant floor.
    pub(crate) fn max_const(self, floor: f64) -> Self {
        if self.value >= floor {
            self
        } else {
            Self::constant(floor)
        }
    }

    pub(crate) fn min_const(self, ceil: f64) -> Self {
        if self.value <= ceil {
            self
        } else {
            Self::constant(ceil)
        }
    }

    pub(crate) fn abs(self) -> Self {
        if self.value >= 0.0 { self } else { -self }
    }
}

impl<const N: usize> Add for Ad<N> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        let mut deriv = self.deriv;
        for (d, r) in deriv.iter_mut().zip(rhs.deriv.iter()) {
            *d += *r;
        }
        Self {
            value: self.value + rhs.value,
            deriv,
        }
    }
}

impl<const N: usize> Sub for Ad<N> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        let mut deriv = self.deriv;
        for (d, r) in deriv.iter_mut().zip(rhs.deriv.iter()) {
            *d -= *r;
        }
        Self {
            value: self.value - rhs.value,
            deriv,
        }
    }
}

impl<const N: usize> Mul for Ad<N> {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        let mut deriv = [0.0; N];
        for i in 0..N {
            deriv[i] = self.deriv[i] * rhs.value + self.value * rhs.deriv[i];
        }
        Self {
            value: self.value * rhs.value,
            deriv,
        }
    }
}

impl<const N: usize> Div for Ad<N> {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        let inv = 1.0 / rhs.value;
        let inv2 = inv * inv;
        let mut deriv = [0.0; N];
        for i in 0..N {
            // (a/b)' = (a' b - a b') / b^2
            deriv[i] = (self.deriv[i] * rhs.value - self.value * rhs.deriv[i]) * inv2;
        }
        Self {
            value: self.value * inv,
            deriv,
        }
    }
}

impl<const N: usize> Neg for Ad<N> {
    type Output = Self;
    fn neg(self) -> Self {
        Self {
            value: -self.value,
            deriv: self.deriv.map(|g| -g),
        }
    }
}

// ---- mixed Ad / f64 operators (constant on the f64 side) ----

impl<const N: usize> Add<f64> for Ad<N> {
    type Output = Self;
    fn add(self, rhs: f64) -> Self {
        Self {
            value: self.value + rhs,
            deriv: self.deriv,
        }
    }
}

impl<const N: usize> Add<Ad<N>> for f64 {
    type Output = Ad<N>;
    fn add(self, rhs: Ad<N>) -> Ad<N> {
        rhs + self
    }
}

impl<const N: usize> Sub<f64> for Ad<N> {
    type Output = Self;
    fn sub(self, rhs: f64) -> Self {
        Self {
            value: self.value - rhs,
            deriv: self.deriv,
        }
    }
}

impl<const N: usize> Sub<Ad<N>> for f64 {
    type Output = Ad<N>;
    fn sub(self, rhs: Ad<N>) -> Ad<N> {
        Ad {
            value: self - rhs.value,
            deriv: rhs.deriv.map(|g| -g),
        }
    }
}

impl<const N: usize> Mul<f64> for Ad<N> {
    type Output = Self;
    fn mul(self, rhs: f64) -> Self {
        Self {
            value: self.value * rhs,
            deriv: self.deriv.map(|g| g * rhs),
        }
    }
}

impl<const N: usize> Mul<Ad<N>> for f64 {
    type Output = Ad<N>;
    fn mul(self, rhs: Ad<N>) -> Ad<N> {
        rhs * self
    }
}

impl<const N: usize> Div<f64> for Ad<N> {
    type Output = Self;
    fn div(self, rhs: f64) -> Self {
        let inv = 1.0 / rhs;
        Self {
            value: self.value * inv,
            deriv: self.deriv.map(|g| g * inv),
        }
    }
}

impl<const N: usize> Div<Ad<N>> for f64 {
    type Output = Ad<N>;
    fn div(self, rhs: Ad<N>) -> Ad<N> {
        Ad::constant(self) / rhs
    }
}

impl<const N: usize> AddAssign for Ad<N> {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl<const N: usize> SubAssign for Ad<N> {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl<const N: usize> MulAssign for Ad<N> {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl<const N: usize> DivAssign for Ad<N> {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}

/// Abstraction over a differentiable scalar so residual primitives can be
/// written once and evaluated with `f64` (plain residual) or `Ad<N>` (exact
/// Jacobian). Branch/segment selection is done on `value()`, so the `f64`
/// instantiation is arithmetically identical to the original scalar code.
pub(crate) trait Scalar:
    Copy
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
    + Add<f64, Output = Self>
    + Sub<f64, Output = Self>
    + Mul<f64, Output = Self>
    + Div<f64, Output = Self>
{
    fn from_f64(v: f64) -> Self;
    fn value(self) -> f64;
    fn exp(self) -> Self;
    fn sqrt(self) -> Self;
    fn powf(self, p: f64) -> Self;
    fn recip(self) -> Self;
    /// Branch-selecting max against a constant floor (clamp lower bound).
    fn max_floor(self, floor: f64) -> Self;
    /// Branch-selecting min against a constant ceiling (clamp upper bound).
    fn min_ceil(self, ceil: f64) -> Self;
    /// Branch-selecting max of two scalars.
    fn max_of(self, other: Self) -> Self;
    /// Branch-selecting min of two scalars.
    fn min_of(self, other: Self) -> Self;
}

impl Scalar for f64 {
    fn from_f64(v: f64) -> Self {
        v
    }
    fn value(self) -> f64 {
        self
    }
    fn exp(self) -> Self {
        f64::exp(self)
    }
    fn sqrt(self) -> Self {
        f64::sqrt(self)
    }
    fn powf(self, p: f64) -> Self {
        f64::powf(self, p)
    }
    fn recip(self) -> Self {
        1.0 / self
    }
    fn max_floor(self, floor: f64) -> Self {
        self.max(floor)
    }
    fn min_ceil(self, ceil: f64) -> Self {
        self.min(ceil)
    }
    fn max_of(self, other: Self) -> Self {
        self.max(other)
    }
    fn min_of(self, other: Self) -> Self {
        self.min(other)
    }
}

impl<const N: usize> Scalar for Ad<N> {
    fn from_f64(v: f64) -> Self {
        Ad::constant(v)
    }
    fn value(self) -> f64 {
        self.value
    }
    fn exp(self) -> Self {
        Ad::exp(self)
    }
    fn sqrt(self) -> Self {
        Ad::sqrt(self)
    }
    fn powf(self, p: f64) -> Self {
        Ad::powf(self, p)
    }
    fn recip(self) -> Self {
        Ad::recip(self)
    }
    fn max_floor(self, floor: f64) -> Self {
        self.max_const(floor)
    }
    fn min_ceil(self, ceil: f64) -> Self {
        self.min_const(ceil)
    }
    fn max_of(self, other: Self) -> Self {
        self.max(other)
    }
    fn min_of(self, other: Self) -> Self {
        self.min(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-10;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= TOL * (1.0 + a.abs().max(b.abs()))
    }

    #[test]
    fn seeds_and_constants() {
        let x = Ad::<2>::variable(3.0, 0);
        assert_eq!(x.value(), 3.0);
        assert_eq!(x.deriv(), &[1.0, 0.0]);
        let c = Ad::<2>::constant(5.0);
        assert_eq!(c.deriv(), &[0.0, 0.0]);
    }

    #[test]
    fn product_rule_and_quotient_rule() {
        // f(x, y) = x * y / (x + y) at (x, y) = (2, 3)
        let x = Ad::<2>::variable(2.0, 0);
        let y = Ad::<2>::variable(3.0, 1);
        let f = x * y / (x + y);

        // value = 6/5 = 1.2
        assert!(close(f.value(), 1.2));
        // df/dx = y^2 / (x+y)^2 = 9/25 ; df/dy = x^2/(x+y)^2 = 4/25
        assert!(close(f.d(0), 9.0 / 25.0));
        assert!(close(f.d(1), 4.0 / 25.0));
    }

    #[test]
    fn exp_ln_sqrt_chain_rule() {
        // f(x) = exp(sqrt(x)) at x = 4 -> exp(2)
        let x = Ad::<1>::variable(4.0, 0);
        let f = x.sqrt().exp();
        assert!(close(f.value(), (2.0_f64).exp()));
        // f'(x) = exp(sqrt(x)) * 0.5/sqrt(x) = exp(2) * 0.25
        assert!(close(f.d(0), (2.0_f64).exp() * 0.25));

        // g(x) = ln(x) at x = 4 -> deriv 1/4
        let g = Ad::<1>::variable(4.0, 0).ln();
        assert!(close(g.value(), (4.0_f64).ln()));
        assert!(close(g.d(0), 0.25));
    }

    #[test]
    fn powf_and_recip() {
        // f(x) = x^1.5 at x = 4 -> 8 ; f' = 1.5 * x^0.5 = 3
        let f = Ad::<1>::variable(4.0, 0).powf(1.5);
        assert!(close(f.value(), 8.0));
        assert!(close(f.d(0), 3.0));

        // recip(x) at x = 2 -> 0.5 ; deriv = -1/4
        let r = Ad::<1>::variable(2.0, 0).recip();
        assert!(close(r.value(), 0.5));
        assert!(close(r.d(0), -0.25));
    }

    #[test]
    fn mixed_f64_operators() {
        let x = Ad::<1>::variable(3.0, 0);
        let f = 10.0 - 2.0 * x + x / 2.0; // 10 - 2x + 0.5x = 10 - 1.5x
        assert!(close(f.value(), 10.0 - 1.5 * 3.0));
        assert!(close(f.d(0), -1.5));
    }

    #[test]
    fn max_selects_branch_derivative() {
        let x = Ad::<1>::variable(1.0, 0);
        let y = Ad::<1>::constant(2.0);
        let m = x.max(y); // y wins -> constant
        assert_eq!(m.value(), 2.0);
        assert_eq!(m.d(0), 0.0);

        let m2 = x.max_const(0.5); // x wins -> keeps derivative
        assert_eq!(m2.value(), 1.0);
        assert_eq!(m2.d(0), 1.0);
    }
}
