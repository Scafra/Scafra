---
name: scafra-api-compatibility
description: Assess compatibility when Scafra public types, traits, macros, signatures, feature flags, crate dependencies or generated APIs change.
---

1. Read AGENTS.md, current manifests, docs/api-stability.md, docs/upgrading.md and
   the compatibility checklist in docs/contributing.md. Verify release phase:
   current 0.1.0 and a proposed policy are not evidence of shipped 1.0 guarantees.
2. Inventory changed public/re-exported/hidden generated surfaces, trait bounds,
   signatures, inference/coherence and observable config/security/lifecycle
   behavior. Compare supported existing consumers and integrations/examples.
3. Classify each change as compatible, breaking or uncertain with evidence;
   explain SemVer implications under the current release phase, migration needs
   and a non-breaking alternative where feasible. Do not silently approve a
   product-level break; follow existing maintainer/ADR requirements.
4. Check optional feature independence, empty facade defaults, additive feature
   unions, dependency closure/versions and direct companion-crate use. Check
   supported macro input and expansion against runtime APIs together. Confirm
   declared Rust 1.88 support with relevant MSRV consumers, not newer Rust alone.
5. Run policy consistency/drift checks and relevant feature/external consumers
   through scafra-verification. Distinguish source/path, pinned Git, staged registry
   and published-package evidence. Do not assume an API diff tool is installed
   or that compilation proves behavioral compatibility.

Output changed contracts and classification, affected consumers/features/MSRV,
version/migration/documentation actions, validation evidence and remaining gaps.
