# Extraction Skill Demo

This example demonstrates the first built-in skill in agent.rs: **Extraction**.

The Extraction skill extracts structured information from unstructured text, with full guardrail enforcement to prevent hallucination.

## What the Skill Does

The `extract` skill takes unstructured text and a target type, returning structured JSON:

| Target | Description | Example Output |
|--------|-------------|----------------|
| `email` | Email addresses | `{"email": ["user@example.com"]}` |
| `url` | URLs and links | `{"url": ["https://example.com"]}` |
| `date` | Dates (ISO format) | `{"date": "2024-01-15"}` |
| `entity` | Named entities | `{"entity": {"people": [], "organizations": [], "locations": []}}` |
| `name` | Person names | `{"name": ["John Smith", "Jane Doe"]}` |

## How to Run

### CLI (Native)

You can call the extraction skill explicitly with the `skill extract` subcommand (recommended for clarity), or use the shorthand `extract` alias:

```bash
# Explicit skill call
cargo run --package agent-native -- skill extract \
  --text "Contact us at hello@agent.rs or visit https://agent.rs" \
  --target "email"

# Shorthand alias (maps to the same skill)
cargo run --package agent-native -- extract \
  --text "Contact us at hello@agent.rs or visit https://agent.rs" \
  --target "email"
```

Expected output:
```json
{
  "email": ["hello@agent.rs"]
}
```

### Browser

```javascript
// In the browser demo
const result = await agent.invokeSkill('extract', {
  text: 'Contact us at hello@agent.rs',
  target: 'email'
});
console.log(result); // { email: ['hello@agent.rs'] }
```

### Edge (Deno)

```typescript
// POST to the edge endpoint
const response = await fetch('http://localhost:8000/skill/extract', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    text: 'Contact us at hello@agent.rs',
    target: 'email'
  })
});
const result = await response.json();
```

## Skill Invocation Protocol

The agent invokes skills via JSON:

```json
{
  "skill": "extract",
  "text": "Contact us at hello@agent.rs",
  "target": "email"
}
```

This is distinct from tool invocation which uses `"tool"` instead of `"skill"`.

## Guardrails in Action

### Successful Extraction

Input:
```json
{
  "skill": "extract",
  "text": "Email us at support@agent.rs or sales@agent.rs",
  "target": "email"
}
```

Output:
```json
{
  "email": ["support@agent.rs", "sales@agent.rs"]
}
```

### Hallucination Rejection

If the LLM tries to return an email that doesn't exist in the source text:

Input text: `"Contact us anytime"`

Invalid LLM output:
```json
{
  "email": ["contact@example.com"]
}
```

**Guardrail rejects**: `HallucinationDetected: 'contact@example.com' not found in source text`

### Schema Violation

If the LLM returns wrong structure:

LLM output:
```json
{
  "emails": ["test@test.com"]
}
```

**Guardrail rejects**: `SchemaViolation: output missing 'email' field`

### Invalid Target

If an unsupported target is requested:

```json
{
  "skill": "extract",
  "text": "Call us at 555-1234",
  "target": "phone"
}
```

**Fails immediately**: `InvalidTarget: unknown target 'phone'`

## Architecture Notes

### Skills vs Tools

| Aspect | Tools | Skills |
|--------|-------|--------|
| Definition | Host-provided capabilities | Contract-based operations |
| Validation | PlausibilityGuard | Schema + Semantic guardrails |
| Execution | Host executes directly | Host executes, core validates |
| Examples | `shell`, `fetch_url`, `read_dom` | `extract` |

### Guardrail Chain

The extraction skill uses this guardrail chain:

1. **Input Validation**: Check text is non-empty, target is valid
2. **Schema Validation**: Output must be valid JSON with target field
3. **Anti-Hallucination**: Extracted values must appear in source text

### Host Responsibilities

The host is responsible for:

1. Executing the skill (calling LLM with extraction prompt)
2. Passing output through `validate_extraction_output()`
3. Returning structured result or explicit error

agent-core never executes skills directly - it only defines contracts and validates outputs.

## Example Scenarios

### Email Extraction
```
Input: "For inquiries, email john@company.com or jane@company.com"
Target: email
Output: {"email": ["john@company.com", "jane@company.com"]}
```

### URL Extraction
```
Input: "Visit our site at https://agent.rs or docs at https://docs.agent.rs"
Target: url
Output: {"url": ["https://agent.rs", "https://docs.agent.rs"]}
```

### Date Extraction
```
Input: "The conference is on March 15, 2024"
Target: date
Output: {"date": "2024-03-15"}
```

### Entity Extraction
```
Input: "Sarah from Google met with John at the New York office"
Target: entity
Output: {
  "entity": {
    "people": ["Sarah", "John"],
    "organizations": ["Google"],
    "locations": ["New York office"]
  }
}
```

### Name Extraction
```
Input: "The report was authored by Dr. Alice Chen and reviewed by Bob Martinez"
Target: name
Output: {"name": ["Dr. Alice Chen", "Bob Martinez"]}
```

### No Match (Valid Result)
```
Input: "This text has no email addresses"
Target: email
Output: {"email": []}
```

## Failure Behavior

All failures are explicit:

- **CLI**: Prints error to stderr, exits with code 1
- **Browser**: Throws `SkillError`, displays in UI
- **Edge**: Returns HTTP 400 with error JSON

No silent failures. No hidden retries. Correctness over convenience.
