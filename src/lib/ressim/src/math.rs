//! Transcendental functions that return the same bits on every target (#62).
//!
//! `f64::powf`, `f64::exp`, `f64::ln` and the rest call the platform's libm: glibc natively, and
//! the toolchain's bundled copy of the `libm` crate on `wasm32-unknown-unknown`. Over 200,000
//! inputs in the ranges the engine uses, glibc differed from wasm in the last bit on 6 % of `pow`
//! and `exp` calls and 3 % of `ln` calls. It is not even one answer natively: glibc picks its
//! `pow` at load time by CPU feature, and its FMA and non-FMA paths disagree with each other.
//! FIM turns a 1-ULP difference into a different Newton path. That is how the 257-knot relperm
//! table (`491a097`) split the native and browser runs of the parity case `buckley-fim` by
//! 1.1e-4 bar, with the same substeps and Newton counts on both.
//!
//! Every engine call goes through here instead, and each function here is the `libm` crate's, the
//! same pure-Rust code on every target. Native runs (the Rust tests, the Python binding, the
//! cross-solver scorecard) therefore compute what the browser computes.
//!
//! What this changes in the browser, measured on the same 200,000 inputs against wasm's `std`:
//!
//! - `powf`, `exp`, `ln`, `cbrt`, `cos`, `acos`, `sinh`, `cosh`, `asinh`, `acosh`: nothing. The
//!   crate's results are bit-identical to what `std` already called.
//! - `hypot`: 13 % of results move by an ulp. The toolchain's copy of `libm` has CORE-MATH's
//!   correctly rounded `hypot`, which the published crate (0.2.16) does not have yet. No native
//!   library reproduces the correctly rounded one (glibc differs on 0.24 %), so agreement across
//!   targets needs one implementation on both. Its only caller is the GMRES Givens rotation.
//!
//! `clippy.toml` disallows the `f64` methods in library code, and `validate-native-binding.sh`
//! runs that lint. `sqrt`, `powi` and `mul_add` stay on `f64`: IEEE requires `sqrt` and `mul_add`
//! to be correctly rounded, and `powi` lowers to the same compiler-builtins routine everywhere.

/// `x` raised to the power `y`.
#[inline]
pub(crate) fn powf(x: f64, y: f64) -> f64 {
    libm::pow(x, y)
}

/// `e^x`.
#[inline]
pub(crate) fn exp(x: f64) -> f64 {
    libm::exp(x)
}

/// Natural logarithm.
#[inline]
pub(crate) fn ln(x: f64) -> f64 {
    libm::log(x)
}

/// Cube root.
#[inline]
pub(crate) fn cbrt(x: f64) -> f64 {
    libm::cbrt(x)
}

/// `sqrt(x² + y²)` without undue overflow or underflow.
#[inline]
pub(crate) fn hypot(x: f64, y: f64) -> f64 {
    libm::hypot(x, y)
}

/// Cosine, in radians.
#[inline]
pub(crate) fn cos(x: f64) -> f64 {
    libm::cos(x)
}

/// Arc cosine, in radians.
#[inline]
pub(crate) fn acos(x: f64) -> f64 {
    libm::acos(x)
}

/// Hyperbolic sine.
#[inline]
pub(crate) fn sinh(x: f64) -> f64 {
    libm::sinh(x)
}

/// Hyperbolic cosine.
#[inline]
pub(crate) fn cosh(x: f64) -> f64 {
    libm::cosh(x)
}

/// Inverse hyperbolic sine.
#[inline]
pub(crate) fn asinh(x: f64) -> f64 {
    libm::asinh(x)
}

/// Inverse hyperbolic cosine.
#[inline]
pub(crate) fn acosh(x: f64) -> f64 {
    libm::acosh(x)
}
