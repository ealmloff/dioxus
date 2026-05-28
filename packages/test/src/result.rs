pub type Result<T> = std::result::Result<T, TesterError>;

/// An error during resolution of a DOM element or making an assertion.
///
/// This normally indicates that the test should fail.
#[derive(Debug, Clone)]
pub enum TesterError {
    /// The given CSS selector had invalid syntax.
    InvalidCssSelector(String),

    /// No element with the test ID, as given by the HTML attribute `data-testid`, was found in the
    /// DOM.
    NoSuchElementWithTestId(String),

    /// No element matching the given CSS selector was found in the DOM.
    NoSuchElementWithCssSelector(String),

    /// An assertion on a test element failed
    AssertionFailure(String),

    /// No pending Dioxus work appeared before the pump timeout elapsed.
    PumpTimeout,

    /// A backend-specific operation failed.
    BackendError(String),
}

impl std::fmt::Display for TesterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TesterError::InvalidCssSelector(selector) => {
                write!(f, "Invalid CSS selector {selector}")
            }
            TesterError::NoSuchElementWithTestId(id) => {
                write!(f, "No such element with test ID {id}")
            }
            TesterError::NoSuchElementWithCssSelector(selector) => {
                write!(f, "No such element with CSS selector {selector}")
            }
            TesterError::AssertionFailure(description) => {
                write!(f, "Failed assertion: {description}")
            }
            TesterError::PumpTimeout => write!(f, "Timed out waiting for Dioxus work"),
            TesterError::BackendError(description) => write!(f, "Backend error: {description}"),
        }
    }
}

impl std::error::Error for TesterError {}
