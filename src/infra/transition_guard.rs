/// Check whether a volume status represents a transitional (in-progress) state.
///
/// Transitional states indicate an operation is already underway,
/// so new actions should be blocked until the current one completes.
pub fn is_volume_in_transition(status: &str) -> bool {
    matches!(
        status,
        "attaching" | "detaching" | "creating" | "deleting" | "uploading" | "downloading"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attaching_is_transition() {
        assert!(is_volume_in_transition("attaching"));
    }

    #[test]
    fn test_detaching_is_transition() {
        assert!(is_volume_in_transition("detaching"));
    }

    #[test]
    fn test_creating_is_transition() {
        assert!(is_volume_in_transition("creating"));
    }

    #[test]
    fn test_deleting_is_transition() {
        assert!(is_volume_in_transition("deleting"));
    }

    #[test]
    fn test_uploading_is_transition() {
        assert!(is_volume_in_transition("uploading"));
    }

    #[test]
    fn test_downloading_is_transition() {
        assert!(is_volume_in_transition("downloading"));
    }

    #[test]
    fn test_available_is_not_transition() {
        assert!(!is_volume_in_transition("available"));
    }

    #[test]
    fn test_in_use_is_not_transition() {
        assert!(!is_volume_in_transition("in-use"));
    }

    #[test]
    fn test_error_is_not_transition() {
        assert!(!is_volume_in_transition("error"));
    }

    #[test]
    fn test_empty_string_is_not_transition() {
        assert!(!is_volume_in_transition(""));
    }

    #[test]
    fn test_case_sensitive() {
        assert!(!is_volume_in_transition("Attaching"));
        assert!(!is_volume_in_transition("DETACHING"));
    }
}
