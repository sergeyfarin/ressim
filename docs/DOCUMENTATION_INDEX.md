# Documentation Index

## Quick Navigation

### 🚀 Getting Started
- **New to the project?** Start with [README.md](README.md)
- **Want to run a simulation?** See [UNIT_REFERENCE.md](UNIT_REFERENCE.md) - API Usage section
- **Setting up the environment?** Check the main README

### 📚 Unit System Documentation

#### [UNIT_REFERENCE.md](UNIT_REFERENCE.md) ⭐ START HERE
Quick reference card with:
- Units at a glance
- Key equations
- Default values
- Common ranges
- API usage examples
- **Perfect for:** Quick lookups, setting up simulations

#### [UNIT_SYSTEM.md](UNIT_SYSTEM.md) - COMPREHENSIVE
Complete unit system documentation with:
- All base and derived units
- Fluid and rock properties
- Grid cell properties
- Complete equations with LaTeX
- Material balance
- Unit conversions
- **Perfect for:** Understanding the system deeply, physics reference

#### [TRANSMISSIBILITY_FACTOR.md](TRANSMISSIBILITY_FACTOR.md) - TECHNICAL DEEP DIVE
Detailed explanation of the 0.001127 factor:
- Formula derivation
- Dimensional analysis
- Darcy's law in oilfield units
- Verification examples
- Sensitivity analysis
- **Perfect for:** Understanding flow calculations, advanced development

#### [UNIT_SYSTEM_CHANGES.md](UNIT_SYSTEM_CHANGES.md) - CHANGE LOG
Complete record of unit system refactoring:
- All modifications made
- File-by-file changes
- Rationale and benefits
- Validation results
- Backward compatibility
- **Perfect for:** Understanding what changed and why

#### [REFACTORING_COMPLETE.md](REFACTORING_COMPLETE.md) - SUMMARY
Executive summary of refactoring:
- What was changed
- Why it was changed
- Results and status
- Recommendations
- Usage guide for different audiences
- **Perfect for:** Overview and high-level understanding

### 🔬 Physics Documentation

#### [PHYSICS_REVIEW.md](PHYSICS_REVIEW.md)
Comprehensive physics analysis:
- Model validation
- Critical issues (capillary pressure, units)
- Logic errors and fixes
- Code quality recommendations
- Priority-ordered improvements
- **Perfect for:** Physics validation, planning enhancements

### 🎯 Purpose-Based Navigation

#### I'm a **User** wanting to set up a simulation
1. Read: [UNIT_REFERENCE.md](UNIT_REFERENCE.md) - API Usage section
2. Check: Default values table
3. Validate: Use the validation checklist
4. Run: Follow example code

#### I'm a **Developer** extending the code
1. Read: [UNIT_SYSTEM.md](UNIT_SYSTEM.md) - Full system overview
2. Check: [TRANSMISSIBILITY_FACTOR.md](TRANSMISSIBILITY_FACTOR.md) for flow calculations
3. Review: Code comments in `src/lib/ressim/src/lib.rs`
4. Reference: [PHYSICS_REVIEW.md](PHYSICS_REVIEW.md) for physics guidance

#### I'm **Maintaining** the simulator
1. Review: [UNIT_SYSTEM_CHANGES.md](UNIT_SYSTEM_CHANGES.md) - What changed
2. Reference: [REFACTORING_COMPLETE.md](REFACTORING_COMPLETE.md) - Implementation notes
3. Validate: Against [UNIT_SYSTEM.md](UNIT_SYSTEM.md) standards
4. Document: Any changes following existing structure

#### I need **Technical Details**
1. Transmissibility? → [TRANSMISSIBILITY_FACTOR.md](TRANSMISSIBILITY_FACTOR.md)
2. All units? → [UNIT_SYSTEM.md](UNIT_SYSTEM.md)
3. Quick lookup? → [UNIT_REFERENCE.md](UNIT_REFERENCE.md)
4. Physics issues? → [PHYSICS_REVIEW.md](PHYSICS_REVIEW.md)

## Unit System At A Glance

```
UNIT SYSTEM: OIL-FIELD UNITS
===========================
Pressure        → bar
Distance        → m
Time            → day
Permeability    → mD (milliDarcy)
Viscosity       → cP (centiPoise)
Compressibility → 1/bar
Saturation      → dimensionless

CRITICAL FACTOR: 0.001127
T [m³/day/bar] = 0.001127 × k[mD] × A[m²] / (L[m] × μ[cP])
See: TRANSMISSIBILITY_FACTOR.md for full explanation
```

## File Organization

```
ressim/
├── README.md                          # Main project documentation
├── UNIT_REFERENCE.md                  # ⭐ Quick reference (START HERE)
├── UNIT_SYSTEM.md                     # Comprehensive units documentation
├── TRANSMISSIBILITY_FACTOR.md         # Technical deep dive
├── UNIT_SYSTEM_CHANGES.md             # Change log
├── REFACTORING_COMPLETE.md            # Summary and recommendations
├── PHYSICS_REVIEW.md                  # Physics validation
├── DOCUMENTATION_INDEX.md             # THIS FILE
├── src/
│   ├── lib/
│   │   └── ressim/
│   │       └── src/
│   │           └── lib.rs             # Main simulator (fully documented)
│   ├── App.svelte                     # Main application
│   ├── main.js                        # Entry point
│   └── 3dview.svelte                  # 3D visualization component
├── index.html                         # HTML entry point
├── package.json                       # Dependencies
├── vite.config.js                     # Build configuration
└── svelte.config.js                   # Svelte configuration
```

## Documentation Metrics

| Document | Lines | Focus | Audience |
|----------|-------|-------|----------|
| UNIT_REFERENCE.md | 350+ | Quick lookup | Everyone |
| UNIT_SYSTEM.md | 700+ | Comprehensive | Developers |
| TRANSMISSIBILITY_FACTOR.md | 200+ | Technical | Advanced devs |
| UNIT_SYSTEM_CHANGES.md | 300+ | Change log | Maintainers |
| REFACTORING_COMPLETE.md | 250+ | Summary | Management |
| PHYSICS_REVIEW.md | 500+ | Physics | Physics team |
| **Total** | **2300+** | **Full documentation** | **Everyone** |

## Key Equations Reference

### Transmissibility
$$T [m³/day/bar] = 0.001127 \times \frac{k_h [mD] \times A [m²]}{L [m]} \times \bar{\lambda} [1/cP]$$

### Well Rate
$$q [m³/day] = PI [m³/day/bar] \times (p_{block} [bar] - BHP [bar])$$

### Corey Relative Permeability
$$k_{rw}(S_w) = \left[\frac{S_w - S_{wc}}{1 - S_{wc} - S_{or}}\right]^{n_w}$$
$$k_{ro}(S_w) = \left[\frac{1 - S_w - S_{or}}{1 - S_{wc} - S_{or}}\right]^{n_o}$$

### Material Balance
$$S_w + S_o = 1.0 \text{ (two-phase system)}$$

## Common Questions

**Q: What units should I use?**
A: Always oil-field units: bar, m, day, mD, cP, 1/bar. See [UNIT_REFERENCE.md](UNIT_REFERENCE.md).

**Q: What is 0.001127?**
A: Conversion factor for transmissibility. See [TRANSMISSIBILITY_FACTOR.md](TRANSMISSIBILITY_FACTOR.md).

**Q: How do I set up a simulation?**
A: See [UNIT_REFERENCE.md](UNIT_REFERENCE.md) - API Usage section.

**Q: What physics is missing?**
A: See [PHYSICS_REVIEW.md](PHYSICS_REVIEW.md) - Capillary pressure is most critical.

**Q: Can I use SI units?**
A: No, simulator uses oil-field units. Conversion factors available in [UNIT_REFERENCE.md](UNIT_REFERENCE.md).

## Recent Changes (Latest Refactoring)

✅ **October 26, 2025** - Unit System Refactoring
- Converted entire simulator to consistent oil-field units
- Created 5 comprehensive documentation files
- Updated all code comments and documentation
- Added examples and validation guides
- See [UNIT_SYSTEM_CHANGES.md](UNIT_SYSTEM_CHANGES.md) for details

## Next Steps

### Short Term
- [ ] Test simulator with benchmark problems
- [ ] Validate output units in frontend
- [ ] Add unit labels to visualization tooltips

### Medium Term
- [ ] Implement capillary pressure (see [PHYSICS_REVIEW.md](PHYSICS_REVIEW.md))
- [ ] Add gravity effects
- [ ] Implement input validation

### Long Term
- [ ] Multi-phase with gas
- [ ] Horizontal wells
- [ ] Advanced boundary conditions

See [REFACTORING_COMPLETE.md](REFACTORING_COMPLETE.md) for detailed recommendations.

## Contributing

When adding new features:
1. Follow oil-field units: bar, m, day, mD, cP, 1/bar
2. Document all new fields with units in code comments
3. Update relevant documentation files
4. Check [PHYSICS_REVIEW.md](PHYSICS_REVIEW.md) for physics guidance
5. Use [UNIT_REFERENCE.md](UNIT_REFERENCE.md) for typical value ranges

## Support

- **Quick question?** Check [UNIT_REFERENCE.md](UNIT_REFERENCE.md)
- **Technical help?** See [UNIT_SYSTEM.md](UNIT_SYSTEM.md)
- **Physics question?** See [PHYSICS_REVIEW.md](PHYSICS_REVIEW.md)
- **Code issue?** Check comments in `src/lib/ressim/src/lib.rs`

## License

All documentation is part of the ressim project.

---

**Last Updated:** October 26, 2025
**Status:** ✅ Unit System Refactoring Complete
**Compilation:** ✅ No errors
**Documentation:** ✅ 2300+ lines across 6 files
