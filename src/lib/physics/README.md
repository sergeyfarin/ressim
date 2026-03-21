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
