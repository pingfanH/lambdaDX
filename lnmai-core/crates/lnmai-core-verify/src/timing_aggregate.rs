//! Ordered timing aggregation shared with the runtime parser.
//! Parsing and exact-number operations stay in pointwise callback adapters.

/// Skip failed entries and add successful exact values in input order.
/// No successful entry means `None`; the first success needs no zero value.
pub fn sum_optional_indexed<T, P, A>(len: usize, parse: P, add: A) -> Option<T>
where
    P: Fn(usize) -> Option<T>,
    A: Fn(T, T) -> T,
{
    let mut total = None;
    let mut index = 0usize;
    while index < len {
        if let Some(value) = parse(index) {
            total = Some(match total {
                Some(sum) => add(sum, value),
                None => value,
            });
        }
        index += 1;
    }
    total
}

/// Quantize exactly once after the complete exact sum, or never for `None`.
#[allow(clippy::manual_map)]
pub fn sum_then_map_indexed<T, U, P, A, Q>(len: usize, parse: P, add: A, quantize: Q) -> Option<U>
where
    P: Fn(usize) -> Option<T>,
    A: Fn(T, T) -> T,
    Q: Fn(T) -> U,
{
    match sum_optional_indexed(len, parse, add) {
        Some(total) => Some(quantize(total)),
        None => None,
    }
}

/// Continue after failed entries and return immediately at the first success.
/// The callback is never invoked for entries after that success.
pub fn first_some_indexed<T, P>(len: usize, parse: P) -> Option<T>
where
    P: Fn(usize) -> Option<T>,
{
    let mut index = 0usize;
    while index < len {
        if let Some(value) = parse(index) {
            return Some(value);
        }
        index += 1;
    }
    None
}
