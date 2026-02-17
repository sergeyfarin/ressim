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
     * Add a well to the simulator
     * Parameters in oil-field units:
     * - i, j, k: grid cell indices (must be within grid bounds)
     * - bhp: bottom-hole pressure [bar] (must be finite, typical: -100 to 2000 bar)
     * - well_radius: wellbore radius [m]
     * - skin: skin factor [dimensionless]
     * - injector: true for injector (injects fluid), false for producer (extracts fluid)
     *
     * Returns Ok(()) on success, or Err(message) if parameters are invalid.
     * Invalid parameters include:
     * - Out-of-bounds grid indices
     * - NaN or Inf values in bhp or pi
     * - Negative productivity index
     * - BHP outside reasonable range
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
     * @returns {any}
     */
    getGridState() {
        const ret = wasm.reservoirsimulator_getGridState(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get last solver warning message (empty string if no warning)
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
     * @returns {any}
     */
    getRateHistory() {
        const ret = wasm.reservoirsimulator_getRateHistory(this.__wbg_ptr);
        return ret;
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
     * Create a new reservoir simulator with oil-field units
     * Grid dimensions: nx, ny, nz (number of cells in each direction)
     * All parameters use: Pressure [bar], Distance [m], Time [day], Permeability [mD], Viscosity [cP]
     * @param {number} nx
     * @param {number} ny
     * @param {number} nz
     */
    constructor(nx, ny, nz) {
        const ret = wasm.reservoirsimulator_new(nx, ny, nz);
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
     * @param {boolean} enabled
     */
    setGravityEnabled(enabled) {
        wasm.reservoirsimulator_setGravityEnabled(this.__wbg_ptr, enabled);
    }
    /**
     * Set initial pressure for all grid cells
     * @param {number} pressure
     */
    setInitialPressure(pressure) {
        wasm.reservoirsimulator_setInitialPressure(this.__wbg_ptr, pressure);
    }
    /**
     * Set initial water saturation for all grid cells
     * @param {number} sat_water
     */
    setInitialSaturation(sat_water) {
        wasm.reservoirsimulator_setInitialSaturation(this.__wbg_ptr, sat_water);
    }
    /**
     * Set initial water saturation per z-layer
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
     * @param {boolean} enabled
     */
    setInjectorEnabled(enabled) {
        wasm.reservoirsimulator_setInjectorEnabled(this.__wbg_ptr, enabled);
    }
    /**
     * Set permeability per layer
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
     * Set permeability with random distribution
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
     * Set permeability with deterministic random distribution using a fixed seed
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
     * @param {boolean} enabled
     */
    setRateControlledWells(enabled) {
        wasm.reservoirsimulator_setRateControlledWells(this.__wbg_ptr, enabled);
    }
    /**
     * Set relative permeability properties
     * @param {number} s_wc
     * @param {number} s_or
     * @param {number} n_w
     * @param {number} n_o
     */
    setRelPermProps(s_wc, s_or, n_w, n_o) {
        const ret = wasm.reservoirsimulator_setRelPermProps(this.__wbg_ptr, s_wc, s_or, n_w, n_o);
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
     * Set stability parameters for the simulation
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
     * Advance simulator by target timestep [days]
     * @param {number} target_dt_days
     */
    step(target_dt_days) {
        wasm.reservoirsimulator_step(this.__wbg_ptr, target_dt_days);
    }
}
if (Symbol.dispose) ReservoirSimulator.prototype[Symbol.dispose] = ReservoirSimulator.prototype.free;

export function set_panic_hook() {
    wasm.set_panic_hook();
}

function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg_Error_8c4e43fe74559d73: function(arg0, arg1) {
            const ret = Error(getStringFromWasm0(arg0, arg1));
            return ret;
        },
        __wbg___wbindgen_debug_string_0bc8482c6e3508ae: function(arg0, arg1) {
            const ret = debugString(arg1);
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_be289d5034ed271b: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_getRandomValues_2a91986308c74a93: function() { return handleError(function (arg0, arg1) {
            globalThis.crypto.getRandomValues(getArrayU8FromWasm0(arg0, arg1));
        }, arguments); },
        __wbg_new_361308b2356cecd0: function() {
            const ret = new Object();
            return ret;
        },
        __wbg_new_3eb36ae241fe6f44: function() {
            const ret = new Array();
            return ret;
        },
        __wbg_set_3f1d0b984ed272ed: function(arg0, arg1, arg2) {
            arg0[arg1] = arg2;
        },
        __wbg_set_f43e577aea94465b: function(arg0, arg1, arg2) {
            arg0[arg1 >>> 0] = arg2;
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
