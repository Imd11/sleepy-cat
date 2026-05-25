pub mod macos;

pub use macos::{accessibility_status, frontmost_app, AccessibilityStatus, FrontmostApp};

#[derive(Clone, Debug, PartialEq)]
pub struct CandidateInput {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl CandidateInput {
    pub fn area(&self) -> f64 {
        self.width * self.height
    }
}

pub fn choose_main_input(candidates: &[CandidateInput]) -> Option<CandidateInput> {
    let valid: Vec<_> = candidates.iter().filter(|c| c.width > 0.0 && c.height > 0.0).collect();
    if valid.is_empty() {
        return None;
    }
    // Sort by area descending
    let mut sorted = valid.to_vec();
    sorted.sort_by(|a, b| b.area().partial_cmp(&a.area()).unwrap());
    Some((*sorted[0]).clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_largest_input_wins() {
        let candidates = vec![
            CandidateInput { x: 0.0, y: 0.0, width: 100.0, height: 100.0 },
            CandidateInput { x: 10.0, y: 10.0, width: 300.0, height: 200.0 },
        ];
        let result = choose_main_input(&candidates).unwrap();
        assert_eq!(result.width, 300.0);
        assert_eq!(result.height, 200.0);
    }

    #[test]
    fn test_zero_size_ignored() {
        let candidates = vec![
            CandidateInput { x: 0.0, y: 0.0, width: 0.0, height: 0.0 },
            CandidateInput { x: 10.0, y: 10.0, width: 50.0, height: 50.0 },
        ];
        let result = choose_main_input(&candidates).unwrap();
        assert_eq!(result.width, 50.0);
    }

    #[test]
    fn test_no_candidate_returns_none() {
        let candidates: Vec<CandidateInput> = vec![];
        let result = choose_main_input(&candidates);
        assert!(result.is_none());
    }
}