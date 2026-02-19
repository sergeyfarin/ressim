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
        onAnalyticalMeta({
            mode: 'depletion',
            shapeFactor: null,
            shapeLabel: 'Peaceman PSS',
        });

        if (!reservoir || timeHistory.length === 0) {
            emitEmpty();
            return;
        }

        const poreVolume = Math.max(1e-9, reservoir.length * reservoir.area * reservoir.porosity);

        // ── Peaceman well PI for oil at initial conditions ─────────────────────
        // Same formula as calculate_well_productivity_index in step.rs
        // r_eq = 0.28 * sqrt(sqrt(kx/ky)*dx² + sqrt(ky/kx)*dy²) / ((kx/ky)^0.25 + (ky/kx)^0.25)
        const kx = Math.max(1e-6, permX);
        const ky = Math.max(1e-6, permY);
        const ratio = kx / ky;
        const r_eq = 0.28
            * Math.sqrt(Math.sqrt(ratio) * cellDx * cellDx + Math.sqrt(1 / ratio) * cellDy * cellDy)
            / (ratio ** 0.25 + (1 / ratio) ** 0.25);
        const rw = Math.max(1e-6, wellRadius);
        const denomPI = Math.max(1e-9, Math.log(Math.max(1 + 1e-9, r_eq / rw)) + wellSkin);

        // k_ro at initial water saturation (Corey model)
        const sw = Math.min(1, Math.max(0, initialSaturation));
        const mobileRange = Math.max(1e-9, 1 - sWc - sOr);
        const se = Math.min(1, Math.max(0, (sw - sWc) / mobileRange));
        const k_ro_swi = Math.max(0, (1 - se) ** nO);

        // Oil PI [m³/(day·bar)] — factor 8.527e-5 converts mD·m / cP to m³/(day·bar)
        const kAvg = Math.sqrt(kx * ky);
        const J_oil = Math.max(1e-12,
            (8.527e-5 * 2 * Math.PI * kAvg * wellboreDz * (k_ro_swi / muO)) / denomPI
            * Math.max(0, depletionRateScale)
        );

        // ── Total compressibility c_t [1/bar] ──────────────────────────────────
        const sO = 1 - sw;
        const c_t = Math.max(1e-12, sO * c_o + sw * c_w + cRock);

        // ── PSS time constant τ = V_pore · c_t / J_oil  [days] ───────────────
        // Derivation: dp_avg/dt = −q / (V_pore · c_t),  q = J_oil · (p_avg − p_wf)
        // → q(t) = q₀ · exp(−t/τ),  τ = V_pore · c_t / J_oil
        const tau = Math.max(1e-6, (poreVolume * c_t) / J_oil);

        // ── Initial rate and maximum recoverable volume ────────────────────────
        const dP = Math.max(0, initialPressure - producerBhp);
        const q0 = J_oil * dP;                        // [m³/day]
        const maxRecoverable = q0 * tau;              // = V_pore · c_t · ΔP [m³]

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
                mode: 'waterflood',
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
