/* @ts-self-types="./simulator.d.ts" */

export class ReservoirSimulator {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ReservoirSimulatorFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_reservoirsimulator_free(ptr, 0);
    }
    /**
     * @returns {number}
     */
    get cumulative_mb_error_m3() {
        const ret = wasm.__wbg_get_reservoirsimulator_cumulative_mb_error_m3(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get cumulative_mb_gas_error_m3() {
        const ret = wasm.__wbg_get_reservoirsimulator_cumulative_mb_gas_error_m3(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {number} i
     * @param {number} j
     * @param {number} k
     * @param {number} bhp
     * @param {number} well_radius
     * @param {number} skin
     * @param {boolean} injector
     */
    add_well(i, j, k, bhp, well_radius, skin, injector) {
        const ret = wasm.reservoirsimulator_add_well(this.__wbg_ptr, i, j, k, bhp, well_radius, skin, injector);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @returns {any}
     */
    getDimensions() {
        const ret = wasm.reservoirsimulator_getDimensions(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {string}
     */
    getLastSolverWarning() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.reservoirsimulator_getLastSolverWarning(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {Float64Array}
     */
    getPressures() {
        const ret = wasm.reservoirsimulator_getPressures(this.__wbg_ptr);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
        return v1;
    }
    /**
     * @returns {any}
     */
    getRateHistory() {
        const ret = wasm.reservoirsimulator_getRateHistory(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {Float64Array}
     */
    getRs() {
        const ret = wasm.reservoirsimulator_getRs(this.__wbg_ptr);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
        return v1;
    }
    /**
     * @returns {Float64Array}
     */
    getSatGas() {
        const ret = wasm.reservoirsimulator_getSatGas(this.__wbg_ptr);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
        return v1;
    }
    /**
     * @returns {Float64Array}
     */
    getSatOil() {
        const ret = wasm.reservoirsimulator_getSatOil(this.__wbg_ptr);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
        return v1;
    }
    /**
     * @returns {Float64Array}
     */
    getSatWater() {
        const ret = wasm.reservoirsimulator_getSatWater(this.__wbg_ptr);
        var v1 = getArrayF64FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 8, 8);
        return v1;
    }
    /**
     * @returns {any}
     */
    getWellState() {
        const ret = wasm.reservoirsimulator_getWellState(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_time() {
        const ret = wasm.reservoirsimulator_get_time(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {number} time_days
     * @param {any} grid_state
     * @param {any} well_state
     * @param {any} rate_history
     */
    loadState(time_days, grid_state, well_state, rate_history) {
        const ret = wasm.reservoirsimulator_loadState(this.__wbg_ptr, time_days, grid_state, well_state, rate_history);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Create a new reservoir simulator with oil-field units
     * Grid dimensions: nx, ny, nz (number of cells in each direction)
     * All parameters use: Pressure [bar], Distance [m], Time [day], Permeability [mD], Viscosity [cP]
     * @param {number} nx
     * @param {number} ny
     * @param {number} nz
     * @param {number} porosity_val
     */
    constructor(nx, ny, nz, porosity_val) {
        const ret = wasm.reservoirsimulator_new(nx, ny, nz, porosity_val);
        this.__wbg_ptr = ret >>> 0;
        ReservoirSimulatorFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @param {number} p_entry
     * @param {number} lambda
     */
    setCapillaryParams(p_entry, lambda) {
        const ret = wasm.reservoirsimulator_setCapillaryParams(this.__wbg_ptr, p_entry, lambda);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} dx
     * @param {number} dy
     * @param {number} dz
     */
    setCellDimensions(dx, dy, dz) {
        const ret = wasm.reservoirsimulator_setCellDimensions(this.__wbg_ptr, dx, dy, dz);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} dx
     * @param {number} dy
     * @param {Float64Array} dz_per_layer
     */
    setCellDimensionsPerLayer(dx, dy, dz_per_layer) {
        const ptr0 = passArrayF64ToWasm0(dz_per_layer, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.reservoirsimulator_setCellDimensionsPerLayer(this.__wbg_ptr, dx, dy, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} c_o
     * @param {number} c_w
     */
    setFluidCompressibilities(c_o, c_w) {
        const ret = wasm.reservoirsimulator_setFluidCompressibilities(this.__wbg_ptr, c_o, c_w);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} rho_o
     * @param {number} rho_w
     */
    setFluidDensities(rho_o, rho_w) {
        const ret = wasm.reservoirsimulator_setFluidDensities(this.__wbg_ptr, rho_o, rho_w);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} mu_o
     * @param {number} mu_w
     */
    setFluidProperties(mu_o, mu_w) {
        const ret = wasm.reservoirsimulator_setFluidProperties(this.__wbg_ptr, mu_o, mu_w);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} mu_g
     * @param {number} c_g
     * @param {number} rho_g
     */
    setGasFluidProperties(mu_g, c_g, rho_g) {
        const ret = wasm.reservoirsimulator_setGasFluidProperties(this.__wbg_ptr, mu_g, c_g, rho_g);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} p_entry
     * @param {number} lambda
     */
    setGasOilCapillaryParams(p_entry, lambda) {
        const ret = wasm.reservoirsimulator_setGasOilCapillaryParams(this.__wbg_ptr, p_entry, lambda);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {boolean} enabled
     */
    setGasRedissolutionEnabled(enabled) {
        wasm.reservoirsimulator_setGasRedissolutionEnabled(this.__wbg_ptr, enabled);
    }
    /**
     * @param {boolean} enabled
     */
    setGravityEnabled(enabled) {
        wasm.reservoirsimulator_setGravityEnabled(this.__wbg_ptr, enabled);
    }
    /**
     * @param {number} sat_gas
     */
    setInitialGasSaturation(sat_gas) {
        wasm.reservoirsimulator_setInitialGasSaturation(this.__wbg_ptr, sat_gas);
    }
    /**
     * @param {Float64Array} sg
     */
    setInitialGasSaturationPerLayer(sg) {
        const ptr0 = passArrayF64ToWasm0(sg, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.reservoirsimulator_setInitialGasSaturationPerLayer(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} pressure
     */
    setInitialPressure(pressure) {
        wasm.reservoirsimulator_setInitialPressure(this.__wbg_ptr, pressure);
    }
    /**
     * @param {number} rs
     */
    setInitialRs(rs) {
        wasm.reservoirsimulator_setInitialRs(this.__wbg_ptr, rs);
    }
    /**
     * @param {number} sat_water
     */
    setInitialSaturation(sat_water) {
        wasm.reservoirsimulator_setInitialSaturation(this.__wbg_ptr, sat_water);
    }
    /**
     * @param {Float64Array} sw
     */
    setInitialSaturationPerLayer(sw) {
        const ptr0 = passArrayF64ToWasm0(sw, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.reservoirsimulator_setInitialSaturationPerLayer(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {string} fluid
     */
    setInjectedFluid(fluid) {
        const ptr0 = passStringToWasm0(fluid, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.reservoirsimulator_setInjectedFluid(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {boolean} enabled
     */
    setInjectorEnabled(enabled) {
        wasm.reservoirsimulator_setInjectorEnabled(this.__wbg_ptr, enabled);
    }
    /**
     * @param {Float64Array} perms_x
     * @param {Float64Array} perms_y
     * @param {Float64Array} perms_z
     */
    setPermeabilityPerLayer(perms_x, perms_y, perms_z) {
        const ptr0 = passArrayF64ToWasm0(perms_x, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArrayF64ToWasm0(perms_y, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passArrayF64ToWasm0(perms_z, wasm.__wbindgen_malloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.reservoirsimulator_setPermeabilityPerLayer(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} min_perm
     * @param {number} max_perm
     */
    setPermeabilityRandom(min_perm, max_perm) {
        const ret = wasm.reservoirsimulator_setPermeabilityRandom(this.__wbg_ptr, min_perm, max_perm);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} min_perm
     * @param {number} max_perm
     * @param {bigint} seed
     */
    setPermeabilityRandomSeeded(min_perm, max_perm, seed) {
        const ret = wasm.reservoirsimulator_setPermeabilityRandomSeeded(this.__wbg_ptr, min_perm, max_perm, seed);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {any} table_js
     */
    setPvtTable(table_js) {
        const ret = wasm.reservoirsimulator_setPvtTable(this.__wbg_ptr, table_js);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {boolean} enabled
     */
    setRateControlledWells(enabled) {
        wasm.reservoirsimulator_setRateControlledWells(this.__wbg_ptr, enabled);
    }
    /**
     * @param {number} s_wc
     * @param {number} s_or
     * @param {number} n_w
     * @param {number} n_o
     * @param {number} k_rw_max
     * @param {number} k_ro_max
     */
    setRelPermProps(s_wc, s_or, n_w, n_o, k_rw_max, k_ro_max) {
        const ret = wasm.reservoirsimulator_setRelPermProps(this.__wbg_ptr, s_wc, s_or, n_w, n_o, k_rw_max, k_ro_max);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} c_r
     * @param {number} depth_reference_m
     * @param {number} b_o
     * @param {number} b_w
     */
    setRockProperties(c_r, depth_reference_m, b_o, b_w) {
        const ret = wasm.reservoirsimulator_setRockProperties(this.__wbg_ptr, c_r, depth_reference_m, b_o, b_w);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} max_sat_change_per_step
     * @param {number} max_pressure_change_per_step
     * @param {number} max_well_rate_change_fraction
     */
    setStabilityParams(max_sat_change_per_step, max_pressure_change_per_step, max_well_rate_change_fraction) {
        wasm.reservoirsimulator_setStabilityParams(this.__wbg_ptr, max_sat_change_per_step, max_pressure_change_per_step, max_well_rate_change_fraction);
    }
    /**
     * @param {number} injector_rate_m3_day
     * @param {number} producer_rate_m3_day
     */
    setTargetWellRates(injector_rate_m3_day, producer_rate_m3_day) {
        const ret = wasm.reservoirsimulator_setTargetWellRates(this.__wbg_ptr, injector_rate_m3_day, producer_rate_m3_day);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} injector_rate_m3_day
     * @param {number} producer_rate_m3_day
     */
    setTargetWellSurfaceRates(injector_rate_m3_day, producer_rate_m3_day) {
        const ret = wasm.reservoirsimulator_setTargetWellSurfaceRates(this.__wbg_ptr, injector_rate_m3_day, producer_rate_m3_day);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {boolean} enabled
     */
    setThreePhaseModeEnabled(enabled) {
        wasm.reservoirsimulator_setThreePhaseModeEnabled(this.__wbg_ptr, enabled);
    }
    /**
     * @param {number} s_wc
     * @param {number} s_or
     * @param {number} s_gc
     * @param {number} s_gr
     * @param {number} s_org
     * @param {number} n_w
     * @param {number} n_o
     * @param {number} n_g
     * @param {number} k_rw_max
     * @param {number} k_ro_max
     * @param {number} k_rg_max
     */
    setThreePhaseRelPermProps(s_wc, s_or, s_gc, s_gr, s_org, n_w, n_o, n_g, k_rw_max, k_ro_max, k_rg_max) {
        const ret = wasm.reservoirsimulator_setThreePhaseRelPermProps(this.__wbg_ptr, s_wc, s_or, s_gc, s_gr, s_org, n_w, n_o, n_g, k_rw_max, k_ro_max, k_rg_max);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {any} table_js
     */
    setThreePhaseScalTables(table_js) {
        const ret = wasm.reservoirsimulator_setThreePhaseScalTables(this.__wbg_ptr, table_js);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} bhp_min
     * @param {number} bhp_max
     */
    setWellBhpLimits(bhp_min, bhp_max) {
        const ret = wasm.reservoirsimulator_setWellBhpLimits(this.__wbg_ptr, bhp_min, bhp_max);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {string} injector_mode
     * @param {string} producer_mode
     */
    setWellControlModes(injector_mode, producer_mode) {
        const ptr0 = passStringToWasm0(injector_mode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(producer_mode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        wasm.reservoirsimulator_setWellControlModes(this.__wbg_ptr, ptr0, len0, ptr1, len1);
    }
    /**
     * @param {number} target_dt_days
     */
    step(target_dt_days) {
        wasm.reservoirsimulator_step(this.__wbg_ptr, target_dt_days);
    }
    /**
     * @param {number} arg0
     */
    set cumulative_mb_error_m3(arg0) {
        wasm.__wbg_set_reservoirsimulator_cumulative_mb_error_m3(this.__wbg_ptr, arg0);
    }
    /**
     * @param {number} arg0
     */
    set cumulative_mb_gas_error_m3(arg0) {
        wasm.__wbg_set_reservoirsimulator_cumulative_mb_gas_error_m3(this.__wbg_ptr, arg0);
    }
}
if (Symbol.dispose) ReservoirSimulator.prototype[Symbol.dispose] = ReservoirSimulator.prototype.free;

export function set_panic_hook() {
    wasm.set_panic_hook();
}

function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg_Error_83742b46f01ce22d: function(arg0, arg1) {
            const ret = Error(getStringFromWasm0(arg0, arg1));
            return ret;
        },
        __wbg_Number_a5a435bd7bbec835: function(arg0) {
            const ret = Number(arg0);
            return ret;
        },
        __wbg___wbindgen_bigint_get_as_i64_447a76b5c6ef7bda: function(arg0, arg1) {
            const v = arg1;
            const ret = typeof(v) === 'bigint' ? v : undefined;
            getDataViewMemory0().setBigInt64(arg0 + 8 * 1, isLikeNone(ret) ? BigInt(0) : ret, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
        },
        __wbg___wbindgen_boolean_get_c0f3f60bac5a78d1: function(arg0) {
            const v = arg0;
            const ret = typeof(v) === 'boolean' ? v : undefined;
            return isLikeNone(ret) ? 0xFFFFFF : ret ? 1 : 0;
        },
        __wbg___wbindgen_debug_string_5398f5bb970e0daa: function(arg0, arg1) {
            const ret = debugString(arg1);
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_in_41dbb8413020e076: function(arg0, arg1) {
            const ret = arg0 in arg1;
            return ret;
        },
        __wbg___wbindgen_is_bigint_e2141d4f045b7eda: function(arg0) {
            const ret = typeof(arg0) === 'bigint';
            return ret;
        },
        __wbg___wbindgen_is_function_3c846841762788c1: function(arg0) {
            const ret = typeof(arg0) === 'function';
            return ret;
        },
        __wbg___wbindgen_is_object_781bc9f159099513: function(arg0) {
            const val = arg0;
            const ret = typeof(val) === 'object' && val !== null;
            return ret;
        },
        __wbg___wbindgen_is_undefined_52709e72fb9f179c: function(arg0) {
            const ret = arg0 === undefined;
            return ret;
        },
        __wbg___wbindgen_jsval_eq_ee31bfad3e536463: function(arg0, arg1) {
            const ret = arg0 === arg1;
            return ret;
        },
        __wbg___wbindgen_jsval_loose_eq_5bcc3bed3c69e72b: function(arg0, arg1) {
            const ret = arg0 == arg1;
            return ret;
        },
        __wbg___wbindgen_number_get_34bb9d9dcfa21373: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'number' ? obj : undefined;
            getDataViewMemory0().setFloat64(arg0 + 8 * 1, isLikeNone(ret) ? 0 : ret, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
        },
        __wbg___wbindgen_string_get_395e606bd0ee4427: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_6ddd609b62940d55: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_call_e133b57c9155d22c: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.call(arg1);
            return ret;
        }, arguments); },
        __wbg_done_08ce71ee07e3bd17: function(arg0) {
            const ret = arg0.done;
            return ret;
        },
        __wbg_getRandomValues_76dfc69825c9c552: function() { return handleError(function (arg0, arg1) {
            globalThis.crypto.getRandomValues(getArrayU8FromWasm0(arg0, arg1));
        }, arguments); },
        __wbg_get_326e41e095fb2575: function() { return handleError(function (arg0, arg1) {
            const ret = Reflect.get(arg0, arg1);
            return ret;
        }, arguments); },
        __wbg_get_unchecked_329cfe50afab7352: function(arg0, arg1) {
            const ret = arg0[arg1 >>> 0];
            return ret;
        },
        __wbg_get_with_ref_key_6412cf3094599694: function(arg0, arg1) {
            const ret = arg0[arg1];
            return ret;
        },
        __wbg_instanceof_ArrayBuffer_101e2bf31071a9f6: function(arg0) {
            let result;
            try {
                result = arg0 instanceof ArrayBuffer;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Uint8Array_740438561a5b956d: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Uint8Array;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_isArray_33b91feb269ff46e: function(arg0) {
            const ret = Array.isArray(arg0);
            return ret;
        },
        __wbg_isSafeInteger_ecd6a7f9c3e053cd: function(arg0) {
            const ret = Number.isSafeInteger(arg0);
            return ret;
        },
        __wbg_iterator_d8f549ec8fb061b1: function() {
            const ret = Symbol.iterator;
            return ret;
        },
        __wbg_length_b3416cf66a5452c8: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_length_ea16607d7b61445b: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_new_5f486cdf45a04d78: function(arg0) {
            const ret = new Uint8Array(arg0);
            return ret;
        },
        __wbg_new_a70fbab9066b301f: function() {
            const ret = new Array();
            return ret;
        },
        __wbg_new_ab79df5bd7c26067: function() {
            const ret = new Object();
            return ret;
        },
        __wbg_next_11b99ee6237339e3: function() { return handleError(function (arg0) {
            const ret = arg0.next();
            return ret;
        }, arguments); },
        __wbg_next_e01a967809d1aa68: function(arg0) {
            const ret = arg0.next;
            return ret;
        },
        __wbg_prototypesetcall_d62e5099504357e6: function(arg0, arg1, arg2) {
            Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), arg2);
        },
        __wbg_set_282384002438957f: function(arg0, arg1, arg2) {
            arg0[arg1 >>> 0] = arg2;
        },
        __wbg_set_6be42768c690e380: function(arg0, arg1, arg2) {
            arg0[arg1] = arg2;
        },
        __wbg_value_21fc78aab0322612: function(arg0) {
            const ret = arg0.value;
            return ret;
        },
        __wbindgen_cast_0000000000000001: function(arg0) {
            // Cast intrinsic for `F64 -> Externref`.
            const ret = arg0;
            return ret;
        },
        __wbindgen_cast_0000000000000002: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_cast_0000000000000003: function(arg0) {
            // Cast intrinsic for `U64 -> Externref`.
            const ret = BigInt.asUintN(64, arg0);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./simulator_bg.js": import0,
    };
}

const ReservoirSimulatorFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_reservoirsimulator_free(ptr >>> 0, 1));

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

function debugString(val) {
    // primitive types
    const type = typeof val;
    if (type == 'number' || type == 'boolean' || val == null) {
        return  `${val}`;
    }
    if (type == 'string') {
        return `"${val}"`;
    }
    if (type == 'symbol') {
        const description = val.description;
        if (description == null) {
            return 'Symbol';
        } else {
            return `Symbol(${description})`;
        }
    }
    if (type == 'function') {
        const name = val.name;
        if (typeof name == 'string' && name.length > 0) {
            return `Function(${name})`;
        } else {
            return 'Function';
        }
    }
    // objects
    if (Array.isArray(val)) {
        const length = val.length;
        let debug = '[';
        if (length > 0) {
            debug += debugString(val[0]);
        }
        for(let i = 1; i < length; i++) {
            debug += ', ' + debugString(val[i]);
        }
        debug += ']';
        return debug;
    }
    // Test for built-in
    const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
    let className;
    if (builtInMatches && builtInMatches.length > 1) {
        className = builtInMatches[1];
    } else {
        // Failed to match the standard '[object ClassName]'
        return toString.call(val);
    }
    if (className == 'Object') {
        // we're a user defined class or Object
        // JSON.stringify avoids problems with cycles, and is generally much
        // easier than looping through ownProperties of `val`.
        try {
            return 'Object(' + JSON.stringify(val) + ')';
        } catch (_) {
            return 'Object';
        }
    }
    // errors
    if (val instanceof Error) {
        return `${val.name}: ${val.message}\n${val.stack}`;
    }
    // TODO we could test for more things here, like `Set`s and `Map`s.
    return className;
}

function getArrayF64FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat64ArrayMemory0().subarray(ptr / 8, ptr / 8 + len);
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

let cachedFloat64ArrayMemory0 = null;
function getFloat64ArrayMemory0() {
    if (cachedFloat64ArrayMemory0 === null || cachedFloat64ArrayMemory0.byteLength === 0) {
        cachedFloat64ArrayMemory0 = new Float64Array(wasm.memory.buffer);
    }
    return cachedFloat64ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function passArrayF64ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 8, 8) >>> 0;
    getFloat64ArrayMemory0().set(arg, ptr / 8);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasm;
function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedFloat64ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('simulator_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
