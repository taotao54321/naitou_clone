#![allow(dead_code)]
#![allow(unused_macros)]

use std::cmp::Ordering;

use num_traits::{WrappingAdd, WrappingSub};

//--------------------------------------------------------------------
// オーバーフロー演算
//
// x = x.wrapping_add(y) を x.wadd(y) と略記する。
//--------------------------------------------------------------------

pub trait WrappingAddExt: WrappingAdd {
    fn wadd(&mut self, rhs: Self) {
        *self = self.wrapping_add(&rhs);
    }
}

impl<T: WrappingAdd> WrappingAddExt for T {}

pub trait WrappingSubExt: WrappingSub {
    fn wsub(&mut self, rhs: Self) {
        *self = self.wrapping_sub(&rhs);
    }
}

impl<T: WrappingSub> WrappingSubExt for T {}

//--------------------------------------------------------------------

#[must_use]
pub fn min_by<T, F: FnOnce(&T, &T) -> Ordering>(v1: T, v2: T, compare: F) -> T {
    match compare(&v1, &v2) {
        Ordering::Greater => v2,
        _ => v1,
    }
}

#[must_use]
pub fn min_by_key<T, F: FnMut(&T) -> K, K: Ord>(v1: T, v2: T, mut f: F) -> T {
    min_by(v1, v2, |v1, v2| f(v1).cmp(&f(v2)))
}

#[must_use]
pub fn max_by<T, F: FnOnce(&T, &T) -> Ordering>(v1: T, v2: T, compare: F) -> T {
    match compare(&v1, &v2) {
        Ordering::Greater => v1,
        _ => v2,
    }
}

#[must_use]
pub fn max_by_key<T, F: FnMut(&T) -> K, K: Ord>(v1: T, v2: T, mut f: F) -> T {
    max_by(v1, v2, |v1, v2| f(v1).cmp(&f(v2)))
}

pub fn chmin<T: Ord>(xmin: &mut T, x: T) -> bool {
    chmin_by(xmin, x, Ord::cmp)
}

pub fn chmin_by<T, F: FnOnce(&T, &T) -> Ordering>(xmin: &mut T, x: T, compare: F) -> bool {
    match compare(&x, xmin) {
        Ordering::Less => {
            *xmin = x;
            true
        }
        _ => false,
    }
}

pub fn chmin_by_key<T, F: FnMut(&T) -> K, K: Ord>(xmin: &mut T, x: T, mut f: F) -> bool {
    chmin_by(xmin, x, |lhs, rhs| f(lhs).cmp(&f(rhs)))
}

pub fn chmax<T: Ord>(xmax: &mut T, x: T) -> bool {
    chmax_by(xmax, x, Ord::cmp)
}

pub fn chmax_by<T, F: FnOnce(&T, &T) -> Ordering>(xmax: &mut T, x: T, compare: F) -> bool {
    match compare(&x, xmax) {
        Ordering::Greater => {
            *xmax = x;
            true
        }
        _ => false,
    }
}

pub fn chmax_by_key<T, F: FnMut(&T) -> K, K: Ord>(xmax: &mut T, x: T, mut f: F) -> bool {
    chmax_by(xmax, x, |lhs, rhs| f(lhs).cmp(&f(rhs)))
}

pub fn opt_chmin<T: Ord>(optmin: &mut Option<T>, x: T) -> bool {
    opt_chmin_by(optmin, x, Ord::cmp)
}

/// optmin が None なら Some(x) にする。
/// optmin が Some(xmin) なら中身に対して chmin_by を行う。
pub fn opt_chmin_by<T, F: FnOnce(&T, &T) -> Ordering>(
    optmin: &mut Option<T>,
    x: T,
    compare: F,
) -> bool {
    match optmin {
        Some(xmin) => match compare(&x, xmin) {
            Ordering::Less => {
                *optmin = Some(x);
                true
            }
            _ => false,
        },
        None => {
            *optmin = Some(x);
            true
        }
    }
}

pub fn opt_chmin_by_key<T, F: FnMut(&T) -> K, K: Ord>(
    optmin: &mut Option<T>,
    x: T,
    mut f: F,
) -> bool {
    opt_chmin_by(optmin, x, |lhs, rhs| f(lhs).cmp(&f(rhs)))
}

pub fn opt_chmax<T: Ord>(optmax: &mut Option<T>, x: T) -> bool {
    opt_chmax_by(optmax, x, Ord::cmp)
}

pub fn opt_chmax_by<T, F: FnOnce(&T, &T) -> Ordering>(
    optmax: &mut Option<T>,
    x: T,
    compare: F,
) -> bool {
    match optmax {
        Some(xmax) => match compare(&x, xmax) {
            Ordering::Greater => {
                *optmax = Some(x);
                true
            }
            _ => false,
        },
        None => {
            *optmax = Some(x);
            true
        }
    }
}

pub fn opt_chmax_by_key<T, F: FnMut(&T) -> K, K: Ord>(
    optmax: &mut Option<T>,
    x: T,
    mut f: F,
) -> bool {
    opt_chmax_by(optmax, x, |lhs, rhs| f(lhs).cmp(&f(rhs)))
}

macro_rules! unwrap_or_break {
    ($option:expr) => {
        match $option {
            Some(x) => x,
            None => break,
        }
    };
}

macro_rules! unwrap_or_continue {
    ($option:expr) => {
        match $option {
            Some(x) => x,
            None => continue,
        }
    };
}

macro_rules! unwrap_or_return {
    ($option:expr, $ret:expr) => {
        match $option {
            Some(x) => x,
            None => return $ret,
        }
    };
    ($option:expr) => {
        unwrap_or_return!($option, ())
    };
}

macro_rules! chk {
    ($cond:expr, $err:expr $(,)?) => {
        if !$cond {
            return ::std::result::Result::Err($err);
        }
    };
}
