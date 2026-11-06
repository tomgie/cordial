//! The component and modal router: handlers match `custom_id` patterns.
//!
//! A pattern has literal text and named parameters in braces, for example
//! `"vote:{poll_id}:{choice}"`. The `custom_id` carries the state, so a handler works after a
//! restart. A `custom_id` has at most 100 characters.

use std::collections::HashMap;
use std::ops::Deref;
use std::str::FromStr;

use crate::interaction::InteractionContext;

/// A parsed `custom_id` pattern.
///
/// [`BotBuilder::component`](crate::BotBuilder::component) and
/// [`BotBuilder::modal`](crate::BotBuilder::modal) parse the patterns of the handlers. Use
/// the same pattern to make the `custom_id`, so that the two cannot disagree.
///
/// ```
/// use cordial_framework::Pattern;
///
/// let pattern = Pattern::parse("vote:{poll}:{choice}").unwrap();
///
/// let custom_id = pattern.format(&[("poll", &12), ("choice", &"yes")]).unwrap();
/// assert_eq!(custom_id, "vote:12:yes");
///
/// let params = pattern.matches(&custom_id).unwrap();
/// assert_eq!(params.parse::<u64>("poll"), Ok(12));
/// assert_eq!(params.get("choice"), Some("yes"));
/// assert!(pattern.matches("vote::yes").is_none());
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    source: String,
    parts: Vec<Part>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Part {
    Literal(String),
    Parameter(String),
}

/// The reason that a pattern is not valid.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PatternError {
    /// A brace is not closed, or a closing brace has no opening brace.
    #[error("the braces in pattern `{0}` do not match")]
    Braces(String),
    /// A parameter has no name.
    #[error("pattern `{0}` has a parameter with no name")]
    EmptyName(String),
    /// Two parameters follow each other with no literal text between them. The match
    /// cannot find the border between them.
    #[error("pattern `{0}` has two parameters with no text between them")]
    AdjacentParameters(String),
    /// Two parameters have the same name.
    #[error("pattern `{0}` uses the parameter name `{1}` two times")]
    DuplicateName(String, String),
}

impl Pattern {
    /// Parses a pattern.
    ///
    /// # Errors
    ///
    /// Gives an error when the braces do not match, a name is empty or used two times, or two
    /// parameters are adjacent.
    pub fn parse(source: &str) -> Result<Self, PatternError> {
        let error_source = || source.to_owned();
        let mut parts = Vec::new();
        let mut rest = source;

        while !rest.is_empty() {
            match rest.find(['{', '}']) {
                Some(index) if rest.as_bytes()[index] == b'}' => {
                    return Err(PatternError::Braces(error_source()));
                }
                Some(index) => {
                    if index > 0 {
                        parts.push(Part::Literal(rest[..index].to_owned()));
                    }
                    let after = &rest[index + 1..];
                    let end = after
                        .find('}')
                        .ok_or_else(|| PatternError::Braces(error_source()))?;
                    let name = &after[..end];
                    if name.contains('{') {
                        return Err(PatternError::Braces(error_source()));
                    }
                    if name.is_empty() {
                        return Err(PatternError::EmptyName(error_source()));
                    }
                    if matches!(parts.last(), Some(Part::Parameter(_))) {
                        return Err(PatternError::AdjacentParameters(error_source()));
                    }
                    let duplicate = parts
                        .iter()
                        .any(|part| matches!(part, Part::Parameter(other) if other == name));
                    if duplicate {
                        return Err(PatternError::DuplicateName(error_source(), name.to_owned()));
                    }
                    parts.push(Part::Parameter(name.to_owned()));
                    rest = &after[end + 1..];
                }
                None => {
                    parts.push(Part::Literal(rest.to_owned()));
                    rest = "";
                }
            }
        }

        Ok(Self {
            source: source.to_owned(),
            parts,
        })
    }

    /// Gives the source text of the pattern.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.source
    }

    /// Matches a `custom_id`. A parameter ends at the first place where the next literal
    /// text matches. A parameter is never empty.
    #[must_use]
    pub fn matches(&self, custom_id: &str) -> Option<Params> {
        let mut params = HashMap::new();
        let mut rest = custom_id;

        for (index, part) in self.parts.iter().enumerate() {
            match part {
                Part::Literal(literal) => rest = rest.strip_prefix(literal.as_str())?,
                Part::Parameter(name) => {
                    let value = match self.parts.get(index + 1) {
                        Some(Part::Literal(next)) => {
                            // Search after the first character, so that the value is not empty.
                            let first = rest.chars().next()?.len_utf8();
                            let end = rest[first..].find(next.as_str())? + first;
                            &rest[..end]
                        }
                        _ => rest,
                    };
                    if value.is_empty() {
                        return None;
                    }
                    params.insert(name.clone(), value.to_owned());
                    rest = &rest[value.len()..];
                }
            }
        }

        rest.is_empty().then_some(Params { values: params })
    }

    /// Fills the parameters to make a `custom_id`.
    ///
    /// Gives `None` when a parameter is absent from `values`.
    #[must_use]
    pub fn format(&self, values: &[(&str, &dyn std::fmt::Display)]) -> Option<String> {
        let mut custom_id = String::new();
        for part in &self.parts {
            match part {
                Part::Literal(literal) => custom_id.push_str(literal),
                Part::Parameter(name) => {
                    let (_, value) = values.iter().find(|(key, _)| key == name)?;
                    custom_id.push_str(&value.to_string());
                }
            }
        }
        Some(custom_id)
    }
}

/// The parameter values of a matched pattern.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Params {
    values: HashMap<String, String>,
}

/// The reason that a parameter is not available.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ParamError {
    /// The pattern has no parameter with this name.
    #[error("the pattern has no parameter `{0}`")]
    Missing(String),
    /// The value does not parse into the type.
    #[error("parameter `{name}` has the value `{value}`, which does not parse")]
    Invalid {
        /// The parameter name.
        name: String,
        /// The value.
        value: String,
    },
}

impl Params {
    /// Gives the text of a parameter.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// Parses a parameter into a type.
    ///
    /// # Errors
    ///
    /// Gives an error when the parameter is absent or does not parse.
    pub fn parse<T: FromStr>(&self, name: &str) -> Result<T, ParamError> {
        let value = self
            .get(name)
            .ok_or_else(|| ParamError::Missing(name.to_owned()))?;
        value.parse().map_err(|_| ParamError::Invalid {
            name: name.to_owned(),
            value: value.to_owned(),
        })
    }
}

/// The context of a component or modal handler: the interaction context and the pattern
/// parameters.
pub struct ComponentContext<S> {
    context: InteractionContext<S>,
    params: Params,
}

impl<S> Clone for ComponentContext<S> {
    fn clone(&self) -> Self {
        Self {
            context: self.context.clone(),
            params: self.params.clone(),
        }
    }
}

impl<S> std::fmt::Debug for ComponentContext<S> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ComponentContext")
            .field("params", &self.params)
            .finish_non_exhaustive()
    }
}

impl<S> Deref for ComponentContext<S> {
    type Target = InteractionContext<S>;

    fn deref(&self) -> &InteractionContext<S> {
        &self.context
    }
}

impl<S: Send + Sync + 'static> ComponentContext<S> {
    pub(crate) fn new(context: InteractionContext<S>, params: Params) -> Self {
        Self { context, params }
    }

    /// Gives the pattern parameters.
    #[must_use]
    pub fn params(&self) -> &Params {
        &self.params
    }

    /// Parses one pattern parameter.
    ///
    /// # Errors
    ///
    /// Gives an error when the parameter is absent or does not parse.
    pub fn param<T: FromStr>(&self, name: &str) -> Result<T, ParamError> {
        self.params.parse(name)
    }

    /// Gives the `custom_id` of the component or the modal.
    #[must_use]
    pub fn custom_id(&self) -> &str {
        let interaction = self.interaction();
        interaction
            .component_data()
            .map(|data| data.custom_id.as_str())
            .or_else(|| interaction.modal_data().map(|data| data.custom_id.as_str()))
            .unwrap_or_default()
    }

    /// Gives the selected values of a select menu. Empty for other components.
    #[must_use]
    pub fn values(&self) -> &[String] {
        self.interaction()
            .component_data()
            .map(|data| data.values.as_slice())
            .unwrap_or_default()
    }

    /// Gives the interaction context.
    #[must_use]
    pub fn into_inner(self) -> InteractionContext<S> {
        self.context
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_parameters() {
        let pattern = Pattern::parse("vote:{poll}:{choice}").unwrap();

        let params = pattern.matches("vote:12:yes:no").unwrap();
        assert_eq!(params.get("poll"), Some("12"));
        assert_eq!(params.get("choice"), Some("yes:no"));
        assert_eq!(params.parse::<u64>("poll"), Ok(12));

        assert!(pattern.matches("vote::yes").is_none());
        assert!(pattern.matches("vote:12:").is_none());
        assert!(pattern.matches("other:12:yes").is_none());
    }

    #[test]
    fn literal_pattern_matches_exactly() {
        let pattern = Pattern::parse("cancel").unwrap();

        assert!(pattern.matches("cancel").is_some());
        assert!(pattern.matches("cancel2").is_none());
    }

    #[test]
    fn rejects_bad_patterns() {
        assert!(matches!(
            Pattern::parse("a:{x"),
            Err(PatternError::Braces(_))
        ));
        assert!(matches!(
            Pattern::parse("a:}"),
            Err(PatternError::Braces(_))
        ));
        assert!(matches!(
            Pattern::parse("a:{}"),
            Err(PatternError::EmptyName(_))
        ));
        assert!(matches!(
            Pattern::parse("{a}{b}"),
            Err(PatternError::AdjacentParameters(_))
        ));
        assert!(matches!(
            Pattern::parse("{a}:{a}"),
            Err(PatternError::DuplicateName(..))
        ));
    }

    #[test]
    fn formats_a_custom_id() {
        let pattern = Pattern::parse("vote:{poll}:{choice}").unwrap();

        let custom_id = pattern.format(&[("poll", &12), ("choice", &"yes")]);

        assert_eq!(custom_id.as_deref(), Some("vote:12:yes"));
        assert_eq!(pattern.format(&[("poll", &12)]), None);
    }
}
