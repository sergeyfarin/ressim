# Scientific-use notice and limitations

ResSim is educational and research software. It is not intended for reserves booking, field or
facility operational decisions, investment decisions, regulatory submissions, or safety-critical
use. Its outputs are examples for learning, comparison, and software research—not qualified
predictions of reservoir performance.

## What the validation establishes

ResSim compares selected behaviors against classical analytical solutions, published benchmark
data, OPM Flow artifacts, and focused regression tests. Each scenario describes the assumptions
and applicability of its reference. The current validation entry points and evidence are indexed in
[the documentation index](DOCUMENTATION_INDEX.md), with solver-specific status in
[the solver comparison summary](SOLVER_COMPARISON_SUMMARY.md),
[black-oil validation](BLACK_OIL_VALIDATION.md), and
[three-phase validation](THREE_PHASE_VALIDATION.md).

Passing these checks establishes only the tested contracts and cases. It does not qualify ResSim as
a general-purpose commercial simulator or establish accuracy for an untested reservoir, fluid,
well configuration, operating schedule, grid, or timestep.

## Important limitations

- Models deliberately simplify reservoir geology, fluid behavior, wells, facilities, controls, and
  numerical algorithms. Scenario reference solutions are valid only under their stated assumptions.
- Results can depend on discretization, timestep control, nonlinear convergence, property inputs,
  boundary conditions, and well controls. Apparent agreement in one plotted quantity is not a full
  validation of the model.
- Bundled published or OPM Flow series have their own provenance and comparison scope. A reference
  overlay is not automatically an acceptance criterion.
- Browser execution and visualization are conveniences, not controls for auditability,
  reproducibility, data governance, or regulated workflows.
- The software is under active development. Known gaps and open work are tracked in
  [GitHub Issues](https://github.com/sergeyfarin/ressim/issues), with strategic order in
  [ROADMAP.md](../ROADMAP.md) and stable evidence in the project documentation.

Use an independently verified, appropriately qualified simulator and professional engineering
review for real assets or decisions. Always preserve inputs, software revision, solver settings,
and validation evidence when using ResSim results in research.
