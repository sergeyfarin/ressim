# PVT Empirical Correlations

This document outlines the standard Black-Oil PVT empirical correlations used in the simulator to generate pressure-dependent fluid properties for reservoir simulation.

The simulator natively operates in consistent oil-field units (Pressure in `bar`, Volume in `sm³` or `m³`, Temperature in `°C`). The internal TS definitions handle the conversions to correlation-specific units (usually `psia`, `°F`, `scf/STB`, `bbl/STB`) and convert the results back.

## Oil Correlations

### 1. Solution Gas-Oil Ratio ($R_s$) and Bubble Point
**Standing (1947)**
Calculates the bubble point pressure $P_b$ (psia) given a solution GOR $R_s$ (scf/STB):
$$ P_b = 18 \left( \frac{R_s}{\gamma_g} \right)^{0.83} \cdot 10^{0.00091 T - 0.0125 API} $$
Rearranged for saturated $R_s$ at pressure $P \le P_b$:
$$ R_s = \gamma_g \left[ \frac{P}{18 \cdot 10^{0.0125 API - 0.00091 T}} \right]^{1.204819} $$
Where:
- $P$ = pressure (psia)
- $T$ = temperature (°F)
- $\gamma_g$ = gas specific gravity (air = 1.0)
- $API$ = oil gravity

### 2. Oil Formation Volume Factor ($B_o$)
**Standing (1947)**
Valid for saturated oil ($P \le P_b$):
$$ B_o = 0.9759 + 0.000120 \left[ R_s \left( \frac{\gamma_g}{\gamma_o} \right)^{0.5} + 1.25 T \right]^{1.2} $$
For undersaturated oil ($P > P_b$), $B_o$ decreases via isothermal compressibility $c_o$:
$$ B_o(P) = B_o(P_b) \cdot \exp(-c_o (P - P_b)) $$
Where $\gamma_o = \frac{141.5}{131.5 + API}$.

**Vasquez–Beggs (1980)**, selectable for the custom black-oil fluid (#61). Standing remains the default:
$$ B_o = 1 + C_1 R_s + C_2 (T - 60) \frac{API}{\gamma_g} + C_3 R_s (T - 60) \frac{API}{\gamma_g} $$
| | $C_1$ | $C_2$ | $C_3$ |
|---|---|---|---|
| API ≤ 30 | 4.677e-4 | 1.751e-5 | −1.811e-8 |
| API > 30 | 4.670e-4 | 1.100e-5 | 1.337e-9 |

The published $\gamma_g$ is the gas gravity at a 100 psig separator; the generator passes its only gas gravity.

**Why the choice exists.** Below $P_b$ a black-oil table is thermodynamically stable only while
$dB_o/dR_s < B_g$. Standing's slope grows with $R_s$ while $B_g$ falls roughly as $1/P$, so
correlation tables trip the `pvt-thermodynamically-unstable` warning just below a high bubble point.
Vasquez–Beggs is linear in $R_s$ and stays stable over the generator's input range, but its $B_{ob}$
differs from Standing's (−30 % to +9 % over API 15–50, 40–150 °C, $P_b$ ≤ 400 bar).
`docs/BLACK_OIL_VALIDATION.md` §4 has the sweep.

**Fitted data ranges**, as usually quoted (e.g. Ahmed, *Reservoir Engineering Handbook*). Outside
these ranges the correlation is an extrapolation:

| Correlation | $P_b$ (psia) | $R_s$ (scf/STB) | $T$ (°F) | API | $\gamma_g$ |
|---|---|---|---|---|---|
| Standing (1947) | 130–7,000 | 20–1,425 | 100–258 | 16.5–63.8 | 0.59–0.95 |
| Vasquez–Beggs (1980) | 50–5,250 | 0–2,199 | 70–295 | 15.3–59.5 | 0.511–1.351 |

At 30 API, γg 0.7 and 80 °C, Standing's $R_s$ reaches 1,425 scf/STB at about $P_b$ = 420 bar. At 50 API it reaches it at about 240 bar.

### 3. Oil Viscosity ($\mu_o$)
**Beggs-Robinson (1975)** for dead and saturated oil.
1. Dead oil viscosity $\mu_{od}$ (cP):
   $$ \mu_{od} = 10^A - 1 $$
   $$ A = 10^z, \quad z = 3.0324 - 0.02023 API - 1.163 \log_{10}(T) $$
2. Saturated oil viscosity $\mu_{os}$ (cP) at $P \le P_b$:
   $$ \mu_{os} = A \cdot \mu_{od}^B $$
   $$ A = 10.715 (R_s + 100)^{-0.515} $$
   $$ B = 5.44 (R_s + 150)^{-0.338} $$

**Vasquez-Beggs** for undersaturated oil ($P > P_b$):
$$ \mu_o = \mu_{ob} \left( \frac{P}{P_b} \right)^m $$
$$ m = 2.6 P^{1.187} \cdot 10^{-5} \cdot \exp(-11.513) $$
(Using $m$ evaluated at the current pressure or bubble point; for simplicity, a constant or linearized formulation is optionally used).

## Gas Correlations

### 4. Pseudo-critical Properties
**Sutton (1985)**
$$ T_{pc} = 169.2 + 349.5 \gamma_g - 74.0 \gamma_g^2 \quad \text{(°R)} $$
$$ P_{pc} = 756.8 - 131.0 \gamma_g - 3.6 \gamma_g^2 \quad \text{(psia)} $$

### 5. Gas Z-Factor ($z$)
**Hall-Yarborough (1973)**
An implicit relation derived from the Carnahan-Starling equation of state.
$$ t = \frac{T_{pc}}{T} $$
$$ A = 0.06125 t \cdot \exp(-1.2 (1-t)^2) $$
$$ B = t (14.76 - 9.76 t + 4.58 t^2) $$
$$ C = t (90.7 - 242.2 t + 42.4 t^2) $$
$$ D = 2.18 + 2.82 t $$
The reduced density parameter $y$ is found via Newton-Raphson iteration on:
$$ f(y) = \frac{y + y^2 + y^3 - y^4}{(1-y)^3} - A P_{pr} - B y^2 + C y^D = 0 $$
$$ z = \frac{A P_{pr}}{y} $$

### 6. Gas Formation Volume Factor ($B_g$)
From the real gas law:
$$ B_g = \frac{P_{sc}}{T_{sc}} \frac{z T}{P} $$
Typically, evaluated at standard conditions (14.7 psia, 60 °F), $B_g$ is derived directly into volume ratios ($m^3/sm^3$).

### 7. Gas Viscosity ($\mu_g$)
**Lee-Gonzalez-Eakin (1966)**
$$ \mu_g = K \cdot \exp\left(X \cdot \rho_g^Y\right) $$
Where $\rho_g$ is gas density in g/cc:
$$ M_g = 28.96 \gamma_g $$
$$ K = \frac{(9.4 + 0.02 M_g) T^{1.5}}{209 + 19 M_g + T} \times 10^{-4} $$
$$ X = 3.5 + \frac{986}{T} + 0.01 M_g $$
$$ Y = 2.4 - 0.2 X $$ 
And $\mu_g$ is natively calculated in cP.
