//! Diagnostic definitions and stable error codes for the Rust-to-Swift compiler frontend.

use rust_swift_ir::SourceSpan;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable diagnostic codes required by the language matrix policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    UnspecializableGenericRejected,
    CustomDropRejected,
    InteriorMutabilityRejected,
    SharedReferenceRejected,
    AliasedMutationRejected,
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
            Self::UnspecializableGenericRejected => "E0011_UNSPECIALIZABLE_GENERIC_REJECTED",
            Self::CustomDropRejected => "E0007_CUSTOM_DROP_REJECTED",
            Self::InteriorMutabilityRejected => "E0012_INTERIOR_MUTABILITY_REJECTED",
            Self::SharedReferenceRejected => "E0013_SHARED_REFERENCE_REJECTED",
            Self::AliasedMutationRejected => "E0006_ALIASED_MUTATION_REJECTED",
        }
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
