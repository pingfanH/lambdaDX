//! Literal, wrapping search over character positions, independent of the UI.

use std::ops::Range;

#[derive(Debug, Clone, Default)]
pub(super) struct Search {
    pattern: String,
    backwards: bool,
}

impl Search {
    pub fn set(&mut self, pattern: String, backwards: bool) {
        self.pattern = pattern;
        self.backwards = backwards;
    }

    pub fn ranges(&self, text: &str) -> Vec<Range<usize>> {
        if self.pattern.is_empty() {
            return Vec::new();
        }
        let length = self.pattern.chars().count();
        let mut bytes = 0;
        let mut chars = 0;
        text.match_indices(&self.pattern)
            .map(|(at, _)| {
                chars += text[bytes..at].chars().count();
                bytes = at;
                chars..chars + length
            })
            .collect()
    }

    pub fn next(
        &self,
        text: &str,
        at: usize,
        opposite: bool,
        count: usize,
    ) -> Option<Range<usize>> {
        let ranges = self.ranges(text);
        if ranges.is_empty() {
            return None;
        }
        let count = (count.max(1) - 1) % ranges.len();
        let index = if self.backwards != opposite {
            let before = ranges.partition_point(|r| r.start < at);
            (before + ranges.len() - 1 - count) % ranges.len()
        } else {
            (ranges.partition_point(|r| r.start <= at) + count) % ranges.len()
        };
        Some(ranges[index].clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_counts_reverse_wrap_and_unicode_use_match_starts() {
        let mut search = Search::default();
        search.set("注释".into(), false);
        let text = "注释 1, 注释 2, 注释";
        assert_eq!(search.ranges(text), [0..2, 6..8, 12..14]);
        assert_eq!(search.next(text, 0, false, 1), Some(6..8));
        assert_eq!(search.next(text, 0, false, 2), Some(12..14));
        assert_eq!(search.next(text, 0, true, 1), Some(12..14));
        search.set("注释".into(), true);
        assert_eq!(search.next(text, 12, false, 1), Some(6..8));
        assert_eq!(search.next(text, 12, true, 1), Some(0..2));
        assert!(search.next("not found", 0, false, 1).is_none());
    }
}
