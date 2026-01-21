# Extraction Skill Demo Walkthrough

This document provides a step-by-step walkthrough of the Extraction skill in action.

## Demo 1: Basic Email Extraction

### Setup

We'll extract email addresses from a simple text input.

### Input

```json
{
  "skill": "extract",
  "text": "Contact us at hello@agent.rs for support",
  "target": "email"
}
```

### Agent Processing

1. **Parse Output**: Agent detects `"skill"` field → `AgentDecision::InvokeSkill`
2. **Validate Input**: Check text is non-empty, target is valid
3. **Host Executes**: Host calls LLM with extraction prompt
4. **Validate Output**: Guardrails check JSON structure and anti-hallucination
5. **Return Result**: Structured JSON returned to user

### Expected Output

```json
{
  "email": "hello@agent.rs"
}
```

---

## Demo 2: Multiple Extractions

### Input

```json
{
  "skill": "extract",
  "text": "Email support@agent.rs for help, or sales@agent.rs for pricing",
  "target": "email"
}
```

### Expected Output

```json
{
  "email": ["support@agent.rs", "sales@agent.rs"]
}
```

---

## Demo 3: Guardrail Rejection - Hallucination

### Input

```json
{
  "skill": "extract",
  "text": "Contact us anytime during business hours",
  "target": "email"
}
```

### Invalid LLM Output (Hallucination)

```json
{
  "email": "contact@business.com"
}
```

### Guardrail Response

```
REJECTED: HallucinationDetected - 'contact@business.com' not found in source text
```

### Correct Behavior

```json
{
  "email": []
}
```

---

## Demo 4: Guardrail Rejection - Schema Violation

### Input

```json
{
  "skill": "extract",
  "text": "Email hello@test.com for info",
  "target": "email"
}
```

### Invalid LLM Output (Wrong Field Name)

```json
{
  "emails": ["hello@test.com"]
}
```

### Guardrail Response

```
REJECTED: SchemaViolation - output missing 'email' field
```

---

## Demo 5: Invalid Target

### Input

```json
{
  "skill": "extract",
  "text": "Call us at 555-1234",
  "target": "phone"
}
```

### Immediate Failure

```
ERROR: InvalidTarget - unknown target 'phone'
```

Note: This fails at input validation, before any LLM call.

---

## Demo 6: URL Extraction

### Input

```json
{
  "skill": "extract",
  "text": "Visit https://agent.rs for documentation and https://github.com/hwclass/agent.rs for source code",
  "target": "url"
}
```

### Expected Output

```json
{
  "url": ["https://agent.rs", "https://github.com/hwclass/agent.rs"]
}
```

---

## Demo 7: Entity Extraction

### Input

```json
{
  "skill": "extract",
  "text": "John Smith from Anthropic presented at the San Francisco conference with Sarah Chen from OpenAI",
  "target": "entity"
}
```

### Expected Output

```json
{
  "entity": {
    "people": ["John Smith", "Sarah Chen"],
    "organizations": ["Anthropic", "OpenAI"],
    "locations": ["San Francisco"]
  }
}
```

---

## Demo 8: Date Extraction

### Input

```json
{
  "skill": "extract",
  "text": "The project deadline is January 15, 2024, with a review on February 1st",
  "target": "date"
}
```

### Expected Output

```json
{
  "date": ["2024-01-15", "2024-02-01"]
}
```

---

## Demo 9: Name Extraction

### Input

```json
{
  "skill": "extract",
  "text": "The report was authored by Dr. Alice Chen and reviewed by Bob Martinez from the legal team",
  "target": "name"
}
```

### Expected Output

```json
{
  "name": ["Dr. Alice Chen", "Bob Martinez"]
}
```

---

## Running the Demos

### CLI

```bash
# Email extraction
cargo run --package agent-native -- \
  --query "Extract emails from: Contact hello@agent.rs for support"

# With skill invocation
cargo run --package agent-native -- skill extract \
  --text "Contact hello@agent.rs for support" \
  --target email
```

### Programmatic (Rust)

```rust
use agent_core::{
    ExtractionInput, ExtractionTarget,
    parse_skill_output, validate_extraction_output
};

// Validate input
let input = ExtractionInput::new(
    "Contact hello@agent.rs for support",
    "email"
);
let target = input.validate()?; // Returns ExtractionTarget::Email

// After LLM returns output, validate it
let llm_output = r#"{"email": "hello@agent.rs"}"#;
let output = parse_skill_output(llm_output, target)?;
validate_extraction_output(&input, &output, target)?;

// Output is now validated and safe to use
println!("{}", output.to_json());
```

---

## Key Takeaways

1. **Skills are contracts**: Input/output schemas are enforced
2. **Guardrails prevent hallucination**: Extracted values must exist in source
3. **Failures are explicit**: No silent errors, no hidden retries
4. **Host-agnostic**: Same validation logic in CLI, browser, and edge
