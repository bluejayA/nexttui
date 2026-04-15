/// SearchFilter operates on `&[Vec<String>]` (cell data) to avoid coupling
/// with the UI layer's Row type. The caller extracts cells before passing.
pub struct SearchFilter {
    active: bool,
    term: String,
}

impl SearchFilter {
    pub fn new() -> Self {
        Self {
            active: false,
            term: String::new(),
        }
    }

    pub fn activate(&mut self) {
        self.active = true;
        self.term.clear();
    }

    pub fn deactivate(&mut self) {
        self.active = false;
        self.term.clear();
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Update search term. Returns the new term for ResourceList.apply_filter().
    pub fn update_term(&mut self, term: &str) -> &str {
        self.term = term.to_string();
        &self.term
    }

    pub fn term(&self) -> &str {
        &self.term
    }

    /// Filter rows by case-insensitive substring match across all cells.
    /// Each row is a Vec<String> of cell values.
    /// Returns indices of matching rows.
    pub fn filter_rows(&self, rows: &[Vec<String>]) -> Vec<usize> {
        if self.term.is_empty() {
            return (0..rows.len()).collect();
        }
        let lower = self.term.to_lowercase();
        rows.iter()
            .enumerate()
            .filter(|(_, cells)| cells.iter().any(|c| c.to_lowercase().contains(&lower)))
            .map(|(i, _)| i)
            .collect()
    }

    /// Find match ranges in text for highlight rendering.
    /// Returns (start, end) char index pairs (not byte offsets) to be safe
    /// with unicode. Caller should use `.chars().skip(start).take(end-start)`
    /// for slicing.
    pub fn match_ranges(&self, text: &str) -> Vec<(usize, usize)> {
        if self.term.is_empty() {
            return Vec::new();
        }
        let text_chars: Vec<char> = text.chars().flat_map(|c| c.to_lowercase()).collect();
        let term_chars: Vec<char> = self.term.chars().flat_map(|c| c.to_lowercase()).collect();

        if term_chars.is_empty() || term_chars.len() > text_chars.len() {
            return Vec::new();
        }

        let mut ranges = Vec::new();
        let mut i = 0;
        while i + term_chars.len() <= text_chars.len() {
            if text_chars[i..i + term_chars.len()] == term_chars[..] {
                ranges.push((i, i + term_chars.len()));
                i += term_chars.len(); // non-overlapping
            } else {
                i += 1;
            }
        }
        ranges
    }
}

impl Default for SearchFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_cell_rows() -> Vec<Vec<String>> {
        vec![
            vec!["web-01".into(), "ACTIVE".into()],
            vec!["web-02".into(), "ERROR".into()],
            vec!["db-01".into(), "ACTIVE".into()],
        ]
    }

    #[test]
    fn test_activate_deactivate() {
        let mut sf = SearchFilter::new();
        assert!(!sf.is_active());
        sf.activate();
        assert!(sf.is_active());
        sf.update_term("web");
        sf.deactivate();
        assert!(!sf.is_active());
        assert_eq!(sf.term(), "");
    }

    #[test]
    fn test_filter_rows_matching() {
        let mut sf = SearchFilter::new();
        sf.activate();
        sf.update_term("web");
        let indices = sf.filter_rows(&sample_cell_rows());
        assert_eq!(indices, vec![0, 1]);
    }

    #[test]
    fn test_filter_rows_case_insensitive() {
        let mut sf = SearchFilter::new();
        sf.activate();
        sf.update_term("active");
        let indices = sf.filter_rows(&sample_cell_rows());
        assert_eq!(indices, vec![0, 2]);
    }

    #[test]
    fn test_filter_rows_empty_term() {
        let sf = SearchFilter::new();
        let indices = sf.filter_rows(&sample_cell_rows());
        assert_eq!(indices, vec![0, 1, 2]);
    }

    #[test]
    fn test_match_ranges() {
        let mut sf = SearchFilter::new();
        sf.update_term("web");
        let ranges = sf.match_ranges("web-01 is a web server");
        assert_eq!(ranges, vec![(0, 3), (12, 15)]);
    }

    #[test]
    fn test_match_ranges_case_insensitive() {
        let mut sf = SearchFilter::new();
        sf.update_term("Web");
        let ranges = sf.match_ranges("WEB-01 web-02");
        assert_eq!(ranges, vec![(0, 3), (7, 10)]);
    }

    #[test]
    fn test_match_ranges_empty_term() {
        let sf = SearchFilter::new();
        assert!(sf.match_ranges("anything").is_empty());
    }

    #[test]
    fn test_match_ranges_no_match() {
        let mut sf = SearchFilter::new();
        sf.update_term("xyz");
        assert!(sf.match_ranges("abc def").is_empty());
    }
}
