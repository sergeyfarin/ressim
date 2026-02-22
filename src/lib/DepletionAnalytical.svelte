<script lang="ts">
    type Reservoir = { length: number; area: number; porosity: number };
    type AnalyticalPoint = { time: number; oilRate: number; waterRate: number; cumulativeOil: number };
    type AnalyticalDataPayload = { production: AnalyticalPoint[] };
    type AnalyticalMetaPayload = {
        mode: 'waterflood' | 'depletion';
        shapeFactor: number | null;
        shapeLabel: string;
    };

    let {
        enabled = false,
        timeHistory = [],
        reservoir,
        initialSaturation = 0.3,
        // Well & reservoir physics (matched to simulator Peaceman model)
        permX = 100.0,        // Horizontal permeability X [mD]
        permY = 100.0,        // Horizontal permeability Y [mD]
        cellDx = 10.0,        // Cell size in X [m]
        cellDy = 10.0,        // Cell size in Y [m]
        wellboreDz = 10.0,    // Total perforated thickness nz*cellDz [m]
        wellRadius = 0.1,     // Wellbore radius [m]
        wellSkin = 0.0,       // Skin factor [-]
        muO = 1.0,            // Oil viscosity [cP]
        sWc = 0.1,            // Connate water saturation [-]
        sOr = 0.1,            // Residual oil saturation [-]
        nO = 2.0,             // Corey exponent for oil [-]
        c_o = 1e-5,           // Oil compressibility [1/bar]
        c_w = 3e-6,           // Water compressibility [1/bar]
        cRock = 1e-6,         // Rock compressibility [1/bar]
        initialPressure = 300.0,  // Initial reservoir pressure [bar]
        producerBhp = 100.0,      // Producer bottom-hole pressure [bar]
        depletionRateScale = 1.0, // Multiplier on initial rate for manual calibration
        onAnalyticalData = () => {},
        onAnalyticalMeta = () => {},
    }: {
        enabled?: boolean;
        timeHistory?: number[];
        reservoir: Reservoir;
        initialSaturation?: number;
        permX?: number;
        permY?: number;
        cellDx?: number;
        cellDy?: number;
        wellboreDz?: number;
        wellRadius?: number;
        wellSkin?: number;
        muO?: number;
        sWc?: number;
        sOr?: number;
        nO?: number;
        c_o?: number;
        c_w?: number;
        cRock?: number;
        initialPressure?: number;
        producerBhp?: number;
        depletionRateScale?: number;
        onAnalyticalData?: (payload: AnalyticalDataPayload) => void;
        onAnalyticalMeta?: (payload: AnalyticalMetaPayload) => void;
    } = $props();

    let analyticalProduction: AnalyticalPoint[] = [];

    function emitEmpty() {
        analyticalProduction = [];
        onAnalyticalData({ production: analyticalProduction });
    }

    function calculateDepletionAnalyticalProduction() {
        if (!reservoir || timeHistory.length === 0) {
            onAnalyticalMeta({ mode: 'depletion', shapeFactor: null, shapeLabel: '' });
            emitEmpty();
            return;
        }

        const poreVolume = Math.max(1e-9, reservoir.length * reservoir.area * reservoir.porosity);

        // ── PSS Productivity Index using Dietz shape factor ─────────────────────
        // J_PSS = DARCY_FACTOR × 2πkh / (μ × [0.5 × ln(4A / (CA × e^(2γ) × rw²))])
        // where A = drainage area, CA = shape factor, γ = Euler constant
        const kx = Math.max(1e-6, permX);
        const ky = Math.max(1e-6, permY);
        const rw = Math.max(1e-6, wellRadius);

        // k_ro at initial water saturation (Corey model)
        const sw = Math.min(1, Math.max(0, initialSaturation));
        const mobileRange = Math.max(1e-9, 1 - sWc - sOr);
        const se = Math.min(1, Math.max(0, (sw - sWc) / mobileRange));
        const k_ro_swi = Math.max(0, (1 - se) ** nO);

        // Drainage area in the horizontal plane
        // reservoir.length = Lx = nx*dx, reservoir.area = Ly*Lz = ny*dy*nz*dz
        // For nz=1: drainage area A = Lx × Ly = reservoir.length × (reservoir.area / wellboreDz)
        const Lx = reservoir.length;
        const Ly = Math.max(cellDy, reservoir.area / Math.max(1e-6, wellboreDz));
        const A_drain = Lx * Ly;

        // Dietz shape factor CA — depends on well position relative to drainage boundary
        // Simple heuristic: estimate from reservoir aspect ratio and well location
        // Well position is at (producerI, producerJ) in grid coordinates.
        // For corner well in rectangle: CA ≈ 2.08-5.38 depending on aspect ratio
        // For center well in square: CA = 30.8828
        // We use the number of cells to infer position: if well is near center, use center CA.
        const nx_cells = Math.max(1, Math.round(Lx / Math.max(1e-6, cellDx)));
        const ny_cells = Math.max(1, Math.round(Ly / Math.max(1e-6, cellDy)));

        // Determine shape factor based on aspect ratio and well position
        let CA: number;
        let shapeLabel: string;
        const aspectRatio = Lx / Math.max(1e-6, Ly);

        // Simple classification based on geometry
        // For 1D-like (ny=1): use 1D slab shape factors
        // For 2D: use rectangle shape factors from Dietz tables
        if (ny_cells <= 1) {
            // 1D slab — well at one end
            // Analytical PSS resistance: R = μL/(3kA) + Peaceman near-well
            // Equivalent CA for 1D slab with well at end:
            // From the formula 0.5*ln(4A/(CA*γ²*rw²)) = L/(3*Lz*dx) + ln(r_eq/rw)
            // We'll compute it directly from the PSS 1D formula instead
            CA = 0; // flag: use direct 1D computation
            shapeLabel = '1D Slab (end well)';
        } else {
            // 2D drainage — classify well position
            // Center well in square: CA = 30.8828
            // Center well in 2:1 rect: CA = 21.84
            // Corner well in square: CA = 4.51
            if (aspectRatio > 0.5 && aspectRatio < 2.0) {
                CA = 30.8828;
                shapeLabel = 'Square (center)';
            } else if (aspectRatio >= 2.0 && aspectRatio < 5.0) {
                CA = 10.84;
                shapeLabel = `Rectangle ${aspectRatio.toFixed(1)}:1`;
            } else if (aspectRatio >= 5.0) {
                CA = 2.36;
                shapeLabel = `Elongated ${aspectRatio.toFixed(0)}:1`;
            } else {
                CA = 10.84;
                shapeLabel = `Rectangle 1:${(1/aspectRatio).toFixed(1)}`;
            }
        }

        // Oil PI [m³/(day·bar)]
        // DARCY_METRIC_FACTOR converts mD·m / cP → m³/(day·bar)
        const DARCY_METRIC_FACTOR = 8.527e-5;
        const kAvg = Math.sqrt(kx * ky);
        let J_oil: number;

        if (CA === 0) {
            // ── 1D PSS: total resistance = reservoir linear + near-wellbore ──
            // R_linear = μ × L / (3 × k × A_cross × DARCY)  [1D slab, well at end]
            // R_well = 1 / PI_Peaceman
            const A_cross = Ly * wellboreDz; // cross-section perpendicular to flow
            const R_linear = muO / (3 * kAvg * A_cross * DARCY_METRIC_FACTOR * (k_ro_swi / muO) * muO);
            // Peaceman near-well resistance
            const ratio = kx / ky;
            const r_eq = 0.28
                * Math.sqrt(Math.sqrt(ratio) * cellDx * cellDx + Math.sqrt(1 / ratio) * cellDy * cellDy)
                / (ratio ** 0.25 + (1 / ratio) ** 0.25);
            const denomPI = Math.max(1e-9, Math.log(Math.max(1 + 1e-9, r_eq / rw)) + wellSkin);
            const PI_peaceman = DARCY_METRIC_FACTOR * 2 * Math.PI * kAvg * wellboreDz * (k_ro_swi / muO) / denomPI;

            // R_linear = L / (3 * kAvg * A_cross * DARCY * (k_ro/mu))
            const R_lin = Lx / (3 * kAvg * A_cross * DARCY_METRIC_FACTOR * (k_ro_swi / muO));
            const R_well = 1 / PI_peaceman;

            J_oil = Math.max(1e-12, 1 / (R_lin + R_well));
        } else {
            // ── 2D PSS with Dietz shape factor ──────────────────────────────────
            const euler_gamma = 0.5772156649;
            const denom = 0.5 * Math.log(4 * A_drain / (CA * Math.exp(2 * euler_gamma) * rw * rw));
            J_oil = Math.max(1e-12,
                DARCY_METRIC_FACTOR * 2 * Math.PI * kAvg * wellboreDz * (k_ro_swi / muO) / Math.max(1e-9, denom + wellSkin)
            );
        }

        onAnalyticalMeta({
            mode: 'depletion',
            shapeFactor: CA,
            shapeLabel,
        });

        // ── Total compressibility c_t [1/bar] ──────────────────────────────────
        const sO = 1 - sw;
        const c_t = Math.max(1e-12, sO * c_o + sw * c_w + cRock);

        // ── PSS time constant τ = V_pore · c_t / J_oil  [days] ───────────────
        // Derivation: dp_avg/dt = −q / (V_pore · c_t),  q = J_oil · (p_avg − p_wf)
        // → q(t) = q₀ · exp(−t/τ),  τ = V_pore · c_t / J_oil
        const tau = Math.max(1e-6, (poreVolume * c_t) / J_oil);

        // ── Initial rate and maximum recoverable volume ────────────────────────
        const dP = Math.max(0, initialPressure - producerBhp);
        const q0 = J_oil * dP * Math.max(0, depletionRateScale);  // scale applied to rate only
        const maxRecoverable = q0 * tau;              // = V_pore · c_t · ΔP · scale [m³]

        analyticalProduction = timeHistory.map((t) => {
            const boundedTime = Math.max(0, Number(t) || 0);
            const exponent = Math.min(700, boundedTime / tau);
            const expTerm = Math.exp(-exponent);
            const oilRate = q0 * expTerm;
            const cumulativeOil = maxRecoverable * (1 - expTerm);

            return {
                time: boundedTime,
                oilRate,
                waterRate: 0,
                cumulativeOil,
            };
        });

        onAnalyticalData({ production: analyticalProduction });
    }

    $effect(() => {
        if (!enabled) {
            onAnalyticalMeta({
                mode: 'depletion',
                shapeFactor: null,
                shapeLabel: '',
            });
            emitEmpty();
        }
    });

    $effect(() => {
        if (enabled && timeHistory.length > 0 && reservoir) {
            calculateDepletionAnalyticalProduction();
        }
    });
</script>
