//! Ordered, fallible collection shared with the runtime slide timing-point parser.
//! The callback owns each result; indexing the caller's input stays in its adapter.

/// Call each index once in ascending order, stopping at the first error.
/// Successful values are appended in that same order; an error discards the prefix.
#[allow(clippy::question_mark)]
pub fn try_map_indexed<T, E, F>(len: usize, parse: F) -> Result<Vec<T>, E>
where
    F: Fn(usize) -> Result<T, E>,
{
    let mut result = Vec::new();
    let mut index = 0usize;
    while index < len {
        let value = match parse(index) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        result.push(value);
        index += 1;
    }
    Ok(result)
}
