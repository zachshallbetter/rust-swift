//! Diagnostic definitions and stable error codes for the Rust-to-Swift compiler frontend.

use rust_swift_ir::SourceSpan;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable diagnostic codes required by the language matrix policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DiagnosticCode {
    UnsafeRejected,
    AsyncRejected,
    RawPointerRejected,
    MacroRejected,
    DynamicDispatchRejected,
    ComplexTraitRejected,
    ClosureBorrowRejected,
    UnsupportedTypeRejected,
    SyntaxError,
    AliasedMutationRejected,
    UnspecializableGenericRejected,
    InteriorMutabilityRejected,
    SharedReferenceRejected,
    CustomDropRejected,
    Int128UnsupportedRejected,
}

impl DiagnosticCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UnsafeRejected => "E0001_UNSAFE_REJECTED",
            Self::AsyncRejected => "E0002_ASYNC_REJECTED",
            Self::RawPointerRejected => "E0003_RAW_POINTER_REJECTED",
            Self::MacroRejected => "E0004_MACRO_REJECTED",
            Self::DynamicDispatchRejected => "E0005_DYNAMIC_DISPATCH_REJECTED",
            Self::ComplexTraitRejected => "E0006_COMPLEX_TRAIT_REJECTED",
            Self::ClosureBorrowRejected => "E0007_CLOSURE_BORROW_REJECTED",
            Self::UnsupportedTypeRejected => "E0008_UNSUPPORTED_TYPE_REJECTED",
            Self::SyntaxError => "E0009_SYNTAX_ERROR",
            Self::AliasedMutationRejected => "E0010_ALIASED_MUTATION_REJECTED",
            Self::UnspecializableGenericRejected => "E0011_UNSPECIALIZABLE_GENERIC_REJECTED",
            Self::InteriorMutabilityRejected => "E0012_INTERIOR_MUTABILITY_REJECTED",
            Self::SharedReferenceRejected => "E0013_SHARED_REFERENCE_REJECTED",
            Self::CustomDropRejected => "E0014_CUSTOM_DROP_REJECTED",
            Self::Int128UnsupportedRejected => "E0015_INT128_UNSUPPORTED_REJECTED",
        }
    }

    pub fn all_variants() -> &'static [DiagnosticCode] {
        &[
            Self::UnsafeRejected,
            Self::AsyncRejected,
            Self::RawPointerRejected,
            Self::MacroRejected,
            Self::DynamicDispatchRejected,
            Self::ComplexTraitRejected,
            Self::ClosureBorrowRejected,
            Self::UnsupportedTypeRejected,
            Self::SyntaxError,
            Self::AliasedMutationRejected,
            Self::UnspecializableGenericRejected,
            Self::InteriorMutabilityRejected,
            Self::SharedReferenceRejected,
            Self::CustomDropRejected,
            Self::Int128UnsupportedRejected,
        ]
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Structured diagnostic emitting stable code, span, reason, and alternative.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub message: String,
    pub span: SourceSpan,
    pub suggested_alternative: Option<String>,
}

impl Diagnostic {
    pub fn rejected(
        code: DiagnosticCode,
        message: impl Into<String>,
        span: SourceSpan,
        suggested_alternative: impl Into<Option<String>>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            span,
            suggested_alternative: suggested_alternative.into(),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {}:{}:{} - {}",
            self.code, self.span.file, self.span.start_line, self.span.end_line, self.message
        )?;
        if let Some(alt) = &self.suggested_alternative {
            write!(f, " (Alternative: {})", alt)?;
        }
        Ok(())
    }
}

impl std::error::Error for Diagnostic {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_diagnostic_codes_are_strictly_unique() {
        let mut seen_numbers = HashSet::new();
        let mut seen_strings = HashSet::new();

        for code in DiagnosticCode::all_variants() {
            let s = code.as_str();
            assert!(
                seen_strings.insert(s),
                "Duplicate diagnostic string found: {}",
                s
            );

            // Extract numeric prefix between 'E' and '_' (e.g. '0001')
            let num_part = s
                .strip_prefix('E')
                .and_then(|rem| rem.split('_').next())
                .expect("Diagnostic code must follow E<number>_<NAME> format");
            assert!(
                seen_numbers.insert(num_part),
                "Duplicate diagnostic numeric code found: {} (full: {})",
                num_part,
                s
            );
        }

        assert_eq!(seen_strings.len(), 15);
        assert_eq!(seen_numbers.len(), 15);
    }
}
