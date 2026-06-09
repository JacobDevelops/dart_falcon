# Rule Acceptance Criteria Policy

**Version**: M4.0 (Provisional)  
**Status**: PROVISIONAL — Default threshold pending M4.2 pilot calibration  
**Last Updated**: 2026-06-10

This document defines the acceptance criteria for rule implementations in jdlint. All rules must meet these criteria before being marked as "ready for production" or integrated into the main rule set.

## Overview

Rule acceptance is based on **diagnostic accuracy**: the degree to which a rule implementation produces diagnostics that match the upstream reference (dart_code_linter / pyramid_lint) or the rule specification.

Acceptance is determined by three matching criteria:

1. **Rule ID + Line Number** (exact match)
2. **Message Text** (fuzzy match with threshold)
3. **Severity** (exact match)

## Matching Criteria

### 1. Rule ID + Line Number (Exact Match)

For each `/* expect: */` annotation in corpus fixtures, the actual diagnostic must:
- Have the **same rule ID** as specified
- Occur on the **same line number** as the annotation

**Example:**
```dart
void example() {
  dynamic value = 42; /* expect: avoid-dynamic */  // Line 2
}
```

Expected diagnostic: `{rule: "avoid-dynamic", line: 2}`  
Acceptable diagnostic: `{rule: "avoid-dynamic", line: 2, message: "...", severity: "ERROR"}`  
Unacceptable diagnostic: `{rule: "avoid-dynamic", line: 3, ...}` (wrong line) or `{rule: "camel-case-types", line: 2, ...}` (wrong rule)

### 2. Message Text (Fuzzy Match)

Diagnostic messages are compared using **Jaro-Winkler string similarity**.

- **Threshold**: 0.85 (default; pending calibration in M4.2)
- **Status**: PROVISIONAL
- If an `/* expect: rule-name, msg: "..." */` annotation is present, the actual message must achieve ≥ 0.85 similarity
- If no message annotation is present, the message is **not validated** (any message passes)

**Example:**

Expected message: `"Avoid using 'dynamic'; use specific types instead"`  
Actual message: `"Avoid using dynamic; use specific types"`  
Similarity: ~0.94 (passes; ≥ 0.85)

Expected message: `"Avoid using 'dynamic'"`  
Actual message: `"This is a warning message"`  
Similarity: ~0.12 (fails; < 0.85)

### 3. Severity (Exact Match)

Diagnostic severity must match exactly:
- Expected: ERROR → Actual: ERROR ✓
- Expected: WARNING → Actual: WARNING ✓
- Expected: ERROR → Actual: WARNING ✗ (mismatch)

Default severity for all violations: **ERROR**

## Acceptance Threshold

| Metric | Acceptance Level | Status |
|--------|------------------|--------|
| Rule ID + Line Number | 100% exact match | Active |
| Message (Jaro-Winkler) | ≥ 0.85 similarity | **PROVISIONAL** |
| Severity | 100% exact match | Active |
| Good File Diagnostics | 0 violations | Active |

## M4.2 Pilot Calibration Process

**Timeline**: Start of M4.2  
**Objective**: Validate and calibrate the default message threshold (0.85) across representative rule types  
**Outcome**: Finalized threshold and calibration report to be committed to this document

### Pilot Sample

Select **10 representative rules** across the rule complexity spectrum:

**SIMPLE rules (2):**
- Rules with straightforward pattern matching, minimal variance in messages
- Examples: `camel-case-types`, `constant-identifier-names`
- Characteristics: Few parameters, simple violation description

**MEDIUM rules (5):**
- Rules with moderate pattern complexity, some parametric messages
- Examples: `avoid-dynamic`, `prefer-trailing-comma`, `unnecessary-statements`
- Characteristics: Named parameters, contextual messages

**COMPLEX rules (3):**
- Rules with intricate logic, high message variance
- Examples: `avoid-positional-boolean-parameters`, `long-method`, `cognitive-complexity`
- Characteristics: Multiple violation types, contextual/computed messages

### Calibration Procedure

1. **Baseline Measurement**
   - Run validation on all 10 pilot rules with threshold set to 0.85
   - Record message variance statistics per rule
   - Flag any rules with <85% message match rate

2. **Variance Analysis**
   - For each rule, compare actual messages vs. upstream reference
   - Categorize mismatches:
     - Grammar/punctuation only (minor)
     - Paraphrased but semantically equivalent (moderate)
     - Substantially different meaning (major)

3. **Threshold Calibration**
   - If >95% of messages are exact match or trivial variants → raise threshold to **0.95**
   - If 85–95% of messages meet 0.85 threshold → **keep at 0.85** (default)
   - If <80% of messages meet 0.85 threshold → **investigate root causes**:
     - Rule implementation diverging from spec?
     - Upstream reference inconsistent?
     - Threshold inherently inappropriate for rule type?
     - Consider lowering to 0.75 or adjusting rule implementation

4. **Documentation**
   - Record per-rule threshold results in calibration table
   - Document any special cases requiring custom thresholds
   - Update this document with finalized threshold

### Expected Outcomes

- **Finalized threshold** (likely 0.85 or 0.95)
- **Per-rule calibration notes** (if custom thresholds needed)
- **Decision rationale** (included in M4.2 sign-off)

## Acceptance Workflow

### Step 1: Implement Rule

Create rule implementation in `crates/jdlint_rules/src/rules/{rule_name}.rs` and corresponding test fixtures in `crates/jdlint_rules/tests/corpus/{rule_name}/`.

### Step 2: Validate Fixtures

Run corpus validation:
```bash
cargo xtask validate-rules {rule-name}
```

Expected output:
- All annotations matched to diagnostics
- Message similarity ≥ 0.85 (or custom threshold)
- Zero false positives in good.dart files
- Exit code: 0

### Step 3: Archive Sign-Off

When all criteria are met, record approval in the rule metadata:

```rust
/// Rule: avoid-dynamic
/// Status: Accepted (M4.1)
/// Acceptance Date: 2026-06-15
/// Validator: Jacob Sanderson
/// Notes: Message threshold 0.85 applied; 100% pass rate in corpus
pub struct AvoidDynamicRule;
```

### Step 4: Integration

Merge rule into main rule set; enable in default configuration.

## Provisional Status Rationale

The default message threshold of **0.85** is provisional because:

1. **Unknown variance distribution**: Not all rules have been ported/implemented yet; true message variance across the rule set is not yet known
2. **Upstream divergence**: jdlint may intentionally diverge from upstream linters on message formatting; threshold should reflect acceptable divergence
3. **Type variance**: SIMPLE rules may have near-identical messages; COMPLEX rules may have inherent variance
4. **Calibration needed**: M4.2 pilot will measure actual variance and provide data-driven threshold

After M4.2 calibration, this status will change to **ACTIVE** with a final threshold.

## Architect Sign-Off Section

To be completed at **M4.2 start**.

### M4.2 Calibration Sign-Off

**Calibration Date**: [TBD — M4.2 start date]  
**Pilot Sample**: 10 representative rules (see Pilot Sample section above)  
**Calibration Results**:
- SIMPLE rules: [TBD] % exact/near-exact match
- MEDIUM rules: [TBD] % exact/near-exact match
- COMPLEX rules: [TBD] % exact/near-exact match
- Overall pass rate at 0.85 threshold: [TBD] %

**Final Threshold Decision**: [TBD — 0.75, 0.85, 0.95, or custom per-rule]  
**Rationale**: [TBD — document reasoning for final threshold]

**Special Cases**:
- [TBD] Rule(s) requiring custom threshold: [rule_name: threshold]

**Architect**: [Name and signature]  
**Date**: [TBD]  
**Approval**: [Approved / Approved with notes / Deferred]

---

## FAQ

### Q: Why Jaro-Winkler for message matching?
A: Jaro-Winkler is robust to minor variations (word order, punctuation, extra articles) while still catching semantic differences. It's commonly used for fuzzy matching in linting and diagnostics.

### Q: Can I use a different threshold for my rule?
A: Not until M4.2 calibration. After calibration, custom thresholds may be approved on a case-by-case basis with architect sign-off. Rationale: we want consistency until we have data.

### Q: What if my rule's message is intentionally different from upstream?
A: Document it in the rule implementation. If intentional divergence is justified (e.g., better UX, more accurate description), include a note in the rule metadata. The validator can accept it with explicit approval.

### Q: How do I know what threshold my rule passed at?
A: Run `cargo xtask validate-rules {rule-name} --verbose` to see per-message similarity scores. The summary will show the pass rate at the default threshold.

### Q: When should I add message annotations?
A: When the message content is important to the rule's value. For simple violations (e.g., "camel-case-types"), the message is usually generic; don't over-annotate. For complex rules, annotate if you want to catch regressions in message quality.

## Related Documents

- `crates/jdlint_rules/tests/corpus/FIXTURE_FORMAT.md` — Corpus fixture format and annotation syntax
- `crates/jdlint_rules/src/rules/` — Individual rule implementations (see rule metadata)
- `CONTRIBUTING.md` — General contribution guidelines (when available)

## Version History

- **M4.0** (2026-06-10): Initial policy specification; threshold set to 0.85 (provisional)
- **M4.2** (pending): Pilot calibration and final threshold decision
