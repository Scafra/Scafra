# ADR-002: Sustainable compiler and API compatibility policy

Status: **Proposed; explicit maintainer approval required.**
Issue #16 independent review at `264b5c3`; policy revision 2.

## Context

The first proposal would preserve Rust 1.88 throughout 1.x without evidence of
capacity to maintain old compiler/dependency combinations for years. It also
needs to distinguish current implemented API candidates from the eventual 1.0
freeze while #12/#19/#15/#33/#42 remain release dependencies.

## Alternatives

- Strict compiler stability: predictable to users, but source-compatible
  dependency/security evolution can force major releases or maintenance forks.
- Managed minor-release increases: patch floors remain predictable; justified
  compiler updates get review, advance RC/release notes and consumer verification.
- A moving compiler window, such as four stable releases: predictable cadence,
  but imposes continual floor changes and testing obligations absent a need.

## Proposed decision

Recommend managed evolution, preserving the current declared Rust 1.88. Future
1.x minor increases require a documented source/dependency/security need, explicit
maintainer decision, old/new floor and migration notes before final release,
updated shared metadata/CI, and locked/fresh consumer verification on that floor
and current stable. No patch increase, automatic backport commitment, or support
window is promised. Incompatible source/behavior changes still need a major.

Preserve empty defaults and the effective existing standard capability boundary;
allow compatible forwarding/private dependency maintenance. Classify all BootUI
configuration and its aggregate field as experimental at every re-export.
Approve release rules separately from freezing API candidates: #12/#19 must
settle HTTP/limit contracts and #15/#33/#42 must supply release verification.

## Consequences and approval

Users taking minor releases review compiler changes; users retaining compilers
may constrain the Scafra minor without assuming continuing security backports.
Hidden macro/runtime helpers still need compatibility within admitted Cargo
resolutions. The canonical [policy](../api-stability.md) gives enforceable rules;
the [independent review](issue-16-review.md#independent-merge-readiness-review)
records findings and tests. These proposals become accepted only when a named
maintainer records approval; local implementation does not supply that approval.
