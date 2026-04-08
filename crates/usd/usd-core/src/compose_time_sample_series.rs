//! Port of `pxr/usd/sdf/composeTimeSampleSeries.h` — `SdfComposeTimeSampleSeries`.
//!
//! Composes a stronger time-sample series over a weaker one using `value_try_compose_over`
//! (matches the optional compose function in OpenUSD).

use usd_vt::{Value, value_try_compose_over};

fn times_equal_default(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6
}

fn held_index(
    series: &[(f64, Value)],
    iter: usize,
    time: f64,
    eq: impl Fn(f64, f64) -> bool,
) -> usize {
    if iter == series.len() {
        return series.len().saturating_sub(1);
    }
    if !eq(series[iter].0, time) && iter > 0 {
        iter - 1
    } else {
        iter
    }
}

/// Composes `[strong]` over `[weak]` (strong wins for trivial types; dict / array-edit use
/// `value_try_compose_over`). Matches `SdfComposeTimeSampleSeries` in OpenUSD.
pub fn sdf_compose_time_sample_series(
    strong: &[(f64, Value)],
    weak: &[(f64, Value)],
) -> Vec<(f64, Value)> {
    let mut out = Vec::new();
    let eq = times_equal_default;

    if weak.is_empty() {
        return strong.to_vec();
    }
    if strong.is_empty() {
        return weak.to_vec();
    }

    let mut strong_iter = 0usize;
    let mut weak_iter = 0usize;
    let inf = f64::INFINITY;

    while strong_iter < strong.len() || weak_iter < weak.len() {
        let strong_time = if strong_iter == strong.len() {
            inf
        } else {
            strong[strong_iter].0
        };
        let weak_time = if weak_iter == weak.len() {
            inf
        } else {
            weak[weak_iter].0
        };

        if strong_time <= weak_time {
            let s_val = &strong[strong_iter].1;
            let w_idx = held_index(weak, weak_iter, strong_time, eq);
            let w_val = &weak[w_idx].1;
            if let Some(composed) = value_try_compose_over(s_val, w_val) {
                out.push((strong_time, composed));
            } else {
                out.push((strong_time, s_val.clone()));
            }
        } else {
            let w_val = &weak[weak_iter].1;
            let s_idx = held_index(strong, strong_iter, weak_time, eq);
            let s_val = &strong[s_idx].1;
            if let Some(composed) = value_try_compose_over(s_val, w_val) {
                out.push((weak_time, composed));
            }
            // else: non-composing stronger hides weaker — no output
        }

        if strong_iter == strong.len() {
            weak_iter += 1;
        } else if weak_iter == weak.len() {
            strong_iter += 1;
        } else if eq(strong_time, weak_time) {
            strong_iter += 1;
            weak_iter += 1;
        } else if strong_time < weak_time {
            strong_iter += 1;
        } else {
            weak_iter += 1;
        }
    }

    out
}

fn held_index_bool(
    series: &[(f64, bool)],
    iter: usize,
    time: f64,
    eq: impl Fn(f64, f64) -> bool,
) -> usize {
    if iter == series.len() {
        return series.len().saturating_sub(1);
    }
    if !eq(series[iter].0, time) && iter > 0 {
        iter - 1
    } else {
        iter
    }
}

/// Matches `_SamplesInIntervalResolver::_ComposePartialOver` in `stage.cpp`: composes
/// `(time, canCompose)` series using `SdfComposeTimeSampleSeries` with
/// `composeFn(strong, weak) = strong ? optional(weak) : nullopt`.
pub fn sdf_compose_time_sample_series_can_compose(
    strong: &[(f64, bool)],
    weak: &[(f64, bool)],
) -> Vec<(f64, bool)> {
    let mut out = Vec::new();
    let eq = times_equal_default;

    if weak.is_empty() {
        return strong.to_vec();
    }
    if strong.is_empty() {
        return weak.to_vec();
    }

    let mut strong_iter = 0usize;
    let mut weak_iter = 0usize;
    let inf = f64::INFINITY;

    while strong_iter < strong.len() || weak_iter < weak.len() {
        let strong_time = if strong_iter == strong.len() {
            inf
        } else {
            strong[strong_iter].0
        };
        let weak_time = if weak_iter == weak.len() {
            inf
        } else {
            weak[weak_iter].0
        };

        if strong_time <= weak_time {
            let s = strong[strong_iter].1;
            let w_idx = held_index_bool(weak, weak_iter, strong_time, eq);
            let w = weak[w_idx].1;
            let composed = if s { Some(w) } else { None };
            if let Some(c) = composed {
                out.push((strong_time, c));
            } else {
                out.push((strong_time, s));
            }
        } else {
            let w = weak[weak_iter].1;
            let s_idx = held_index_bool(strong, strong_iter, weak_time, eq);
            let s = strong[s_idx].1;
            let composed = if s { Some(w) } else { None };
            if let Some(c) = composed {
                out.push((weak_time, c));
            }
        }

        if strong_iter == strong.len() {
            weak_iter += 1;
        } else if weak_iter == weak.len() {
            strong_iter += 1;
        } else if eq(strong_time, weak_time) {
            strong_iter += 1;
            weak_iter += 1;
        } else if strong_time < weak_time {
            strong_iter += 1;
        } else {
            weak_iter += 1;
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_vt::Dictionary;

    #[test]
    fn empty_weak_returns_strong() {
        let strong = vec![(1.0, Value::from(1i32))];
        let weak: Vec<(f64, Value)> = vec![];
        let r = sdf_compose_time_sample_series(&strong, &weak);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].0, 1.0);
    }

    #[test]
    fn empty_strong_returns_weak() {
        let strong: Vec<(f64, Value)> = vec![];
        let weak = vec![(2.0, Value::from(2i32))];
        let r = sdf_compose_time_sample_series(&strong, &weak);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].0, 2.0);
    }

    #[test]
    fn scalar_non_composing_emits_stronger_only_at_strong_times() {
        let strong = vec![(1.0, Value::from(10i32))];
        let weak = vec![(1.0, Value::from(20i32)), (3.0, Value::from(30i32))];
        let r = sdf_compose_time_sample_series(&strong, &weak);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].0, 1.0);
        assert_eq!(r[0].1.get::<i32>(), Some(&10));
    }

    #[test]
    fn can_compose_series_matches_bool_semantics() {
        let strong = vec![(1.0, true), (3.0, false)];
        let weak = vec![(1.0, false), (2.0, true)];
        let r = sdf_compose_time_sample_series_can_compose(&strong, &weak);
        assert!(!r.is_empty());
    }

    #[test]
    fn dict_composes_at_same_time() {
        let mut d1 = Dictionary::new();
        d1.insert("a", 1i32);
        let mut d2 = Dictionary::new();
        d2.insert("b", 2i32);
        let strong = vec![(1.0, Value::new(d1))];
        let weak = vec![(1.0, Value::new(d2))];
        let r = sdf_compose_time_sample_series(&strong, &weak);
        assert_eq!(r.len(), 1);
        let dict = r[0].1.get::<Dictionary>().unwrap();
        assert_eq!(dict.get_as::<i32>("a"), Some(&1));
        assert_eq!(dict.get_as::<i32>("b"), Some(&2));
    }
}
