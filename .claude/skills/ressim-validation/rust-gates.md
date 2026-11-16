# Rust gate buckets — how `validate-solver-coverage.sh` counts

Reference for the `ressim-validation` skill. The commands are in `SKILL.md`; this file is what
to know before changing a bucket or a test it selects.

The FIM bucket roughly doubled during the F0–F8 repair series, from 14 gates / ~26 s to 17 / ~58 s.
Three filters were added because their modules were in no bucket at all (`fim::linear::`,
`fim::flow_resv::`, and the `fim_repair_` lifecycle group), and one of them dominates the cost:
`fim_repair_` runs ~31 s, almost entirely
`fim_repair_multi_completion_gravity_stays_physical_on_both_solvers`, which steps a 30x1x20
gravity section for 12 days across four solver/completion configurations. That is the price of
covering the multi-completion geometry on both backends (#10); if the bucket ever needs to get
back under ~30 s, that test is the thing to shorten, not the filters to drop.

The script builds the test target first (a compile break fails as a build error before
any bucket runs) and prints a `gate ok: '<filter>' ran N test(s)` line per filter. A
filter that matches **no** tests is a hard failure — `cargo test <filter>` exits 0 when
nothing matches, so without that check a renamed or deleted test would silently turn its
gate line into a no-op that still reported success. If you rename a test, update the
filter in the script.

A filter that matches **only `#[ignore]`d tests** is also a hard failure (#13): the count
is of *passed* tests, not matched ones, so a release-only replay cannot masquerade as a
gate that ran. `gate ok` lines report ignored tests separately when both are present.
