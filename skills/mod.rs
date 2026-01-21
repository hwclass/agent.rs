//! Skills Module
//!
//! Skills are contracts that define structured operations the agent can invoke.
//! Unlike tools (which are host-provided capabilities), skills are:
//!
//! - **Contract-based**: Defined by explicit input/output schemas
//! - **Guardrail-enforced**: Outputs are validated before acceptance
//! - **Host-agnostic**: Same behavior across CLI, browser, and edge
//!
//! ## Available Skills
//!
//! - `extract` - Extract structured information from unstructured text

pub mod extraction;

// Re-export commonly used types
pub use extraction::{
    ExtractionInput,
    ExtractionOutput,
    ExtractionTarget,
    SkillError,
    SkillMetadata,
    SkillRequest,
    SkillResponse,
    SkillResult,
    EXTRACTION_SKILL,
    parse_skill_output,
    validate_extraction_output,
};
