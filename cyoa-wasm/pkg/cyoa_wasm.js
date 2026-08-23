/* @ts-self-types="./cyoa_wasm.d.ts" */

/**
 * A CYOA story engine instance.
 *
 * Each engine is independent — state is never shared between instances.
 * Create one per story (main quest, side quests, etc.).
 */
export class WasmEngine {
    static __wrap(ptr) {
        const obj = Object.create(WasmEngine.prototype);
        obj.__wbg_ptr = ptr;
        WasmEngineFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmEngineFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmengine_free(ptr, 0);
    }
    /**
     * List all event IDs in the story (useful for quest logs).
     * @returns {string[]}
     */
    availableEvents() {
        const ret = wasm.wasmengine_availableEvents(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]);
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Check if an event by ID is reachable from the current story graph.
     * @param {string} id
     * @returns {boolean}
     */
    canAccessEvent(id) {
        const ptr0 = passStringToWasm0(id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmengine_canAccessEvent(this.__wbg_ptr, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Zero-copy: returns a `Uint8Array` view of all current choice texts,
     * newline-separated.
     *
     * The bytes live in WASM linear memory and are valid until the next
     * method call on this engine.
     * @returns {Uint8Array}
     */
    currentChoicesBytes() {
        const ret = wasm.wasmengine_currentChoicesBytes(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get the current event's internal ID (name).
     * @returns {string}
     */
    currentEventId() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmengine_currentEventId(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Zero-copy: returns a `Uint8Array` view of all current event text
     * paragraphs, newline-separated.
     *
     * The bytes live in WASM linear memory and are valid until the next
     * method call on this engine. Use `TextDecoder` to decode:
     *
     * ```js
     * const bytes = engine.currentEventTextBytes();
     * const text = new TextDecoder().decode(bytes);
     * ```
     * @returns {Uint8Array}
     */
    currentEventTextBytes() {
        const ret = wasm.wasmengine_currentEventTextBytes(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get the current event as a JS object:
     * ```json
     * { "id": "old_ruins", "text": ["...", "..."], "choices": ["...", "..."] }
     * ```
     * @returns {any}
     */
    getCurrentEvent() {
        const ret = wasm.wasmengine_getCurrentEvent(this.__wbg_ptr);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
    }
    /**
     * Get the choice history as an array of `{ eventId, choiceIndex, choiceText }`.
     * @returns {any}
     */
    getHistory() {
        const ret = wasm.wasmengine_getHistory(this.__wbg_ptr);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
    }
    /**
     * Get a stat value by name. Returns 0 if the stat doesn't exist.
     * @param {string} name
     * @returns {bigint}
     */
    getStat(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmengine_getStat(this.__wbg_ptr, ptr0, len0);
        return ret;
    }
    /**
     * Get the full player state as a JSON string.
     * Use this for save files. The JSON contains `stats`, `flags`, and `tags`.
     *
     * The returned string is valid until the next method call on this engine.
     * @returns {string}
     */
    getStateJson() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmengine_getStateJson(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * List all runtime flags currently set.
     * @returns {string[]}
     */
    listFlags() {
        const ret = wasm.wasmengine_listFlags(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]);
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Get stats as a JS object mapping stat names to current values.
     * @returns {any}
     */
    listStats() {
        const ret = wasm.wasmengine_listStats(this.__wbg_ptr);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
    }
    /**
     * List story-level tags declared in the story (static metadata).
     * These are distinct from runtime tags (see [`listTags`](Self::listTags)).
     * @returns {string[]}
     */
    listStoryTags() {
        const ret = wasm.wasmengine_listStoryTags(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]);
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * List runtime tags currently applied during play.
     * @returns {string[]}
     */
    listTags() {
        const ret = wasm.wasmengine_listTags(this.__wbg_ptr);
        var v1 = getArrayJsValueFromWasm0(ret[0], ret[1]);
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Make a choice at the given index.
     * Returns `{ effectText: string[], gameStateChanged: boolean }`.
     *
     * `choiceIndex` is 0-based, referencing only choices visible to the player
     * (prerequisites are already filtered).
     * @param {number} choice_index
     * @returns {any}
     */
    makeChoice(choice_index) {
        const ret = wasm.wasmengine_makeChoice(this.__wbg_ptr, choice_index);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
    }
    /**
     * Create an engine directly from compiled bytecode bytes.
     *
     * Alternatively, use [`WasmStoryCatalog::createEngine`] to create engines
     * from a catalog of tagged stories.
     * @param {Uint8Array} bytes
     */
    constructor(bytes) {
        const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmengine_new(ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0];
        WasmEngineFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Restore state from a JSON string produced by [`getStateJson`](Self::get_state_json).
     * @param {string} json
     */
    setStateJson(json) {
        const ptr0 = passStringToWasm0(json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmengine_setStateJson(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
}
if (Symbol.dispose) WasmEngine.prototype[Symbol.dispose] = WasmEngine.prototype.free;

/**
 * A registry of compiled stories with tag-based filtering.
 *
 * Game engines (especially random event systems) use this to discover
 * and filter stories by tags before creating a `WasmEngine` to play them.
 */
export class WasmStoryCatalog {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmStoryCatalogFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmstorycatalog_free(ptr, 0);
    }
    /**
     * Create an `Engine` for the story at the given index.
     * Returns `null` if the index is out of bounds.
     * @param {number} index
     * @returns {WasmEngine | undefined}
     */
    createEngine(index) {
        const ret = wasm.wasmstorycatalog_createEngine(this.__wbg_ptr, index);
        return ret === 0 ? undefined : WasmEngine.__wrap(ret);
    }
    /**
     * Find a story by name and create an `Engine` for it.
     * Returns `null` if no story with that name is registered.
     * @param {string} name
     * @returns {WasmEngine | undefined}
     */
    createEngineByName(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmstorycatalog_createEngineByName(this.__wbg_ptr, ptr0, len0);
        return ret === 0 ? undefined : WasmEngine.__wrap(ret);
    }
    /**
     * Whether the catalog is empty.
     * @returns {boolean}
     */
    isEmpty() {
        const ret = wasm.wasmstorycatalog_isEmpty(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Number of registered stories.
     * @returns {number}
     */
    len() {
        const ret = wasm.wasmstorycatalog_len(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * List all registered stories as an array of `{ name, tags }` objects.
     *
     * ```json
     * [{"name": "ForestAdventure", "tags": ["fantasy", "exploration"]}, ...]
     * ```
     * @returns {any}
     */
    listStories() {
        const ret = wasm.wasmstorycatalog_listStories(this.__wbg_ptr);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
    }
    /**
     * Create an empty catalog.
     */
    constructor() {
        const ret = wasm.wasmstorycatalog_new();
        this.__wbg_ptr = ret;
        WasmStoryCatalogFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Register a compiled story from bytecode bytes.
     *
     * The bytecode must have been produced by `cyoa compile`.
     * Story name and tags are extracted from the bytecode at registration.
     * @param {Uint8Array} bytes
     */
    register(bytes) {
        const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmstorycatalog_register(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Find all stories that have ALL of the specified tags.
     *
     * `tagsJson` is a JSON array of tag strings, e.g. `"[\"fantasy\", \"exploration\"]"`.
     * @param {string} tags_json
     * @returns {any}
     */
    storiesWithAllTags(tags_json) {
        const ptr0 = passStringToWasm0(tags_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmstorycatalog_storiesWithAllTags(this.__wbg_ptr, ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
    }
    /**
     * Find all stories that have ANY of the specified tags.
     *
     * `tagsJson` is a JSON array of tag strings, e.g. `"[\"fantasy\", \"combat\"]"`.
     * @param {string} tags_json
     * @returns {any}
     */
    storiesWithAnyTags(tags_json) {
        const ptr0 = passStringToWasm0(tags_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmstorycatalog_storiesWithAnyTags(this.__wbg_ptr, ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
    }
    /**
     * Find all stories that have a specific tag.
     * @param {string} tag
     * @returns {any}
     */
    storiesWithTag(tag) {
        const ptr0 = passStringToWasm0(tag, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmstorycatalog_storiesWithTag(this.__wbg_ptr, ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
    }
}
if (Symbol.dispose) WasmStoryCatalog.prototype[Symbol.dispose] = WasmStoryCatalog.prototype.free;
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg_String_8564e559799eccda: function(arg0, arg1) {
            const ret = String(arg1);
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_debug_string_a57024b9c6e4a48b: function(arg0, arg1) {
            const ret = debugString(arg1);
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
            getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_bb96b2010945f0bc: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_new_116be93542d39019: function() {
            const ret = new Array();
            return ret;
        },
        __wbg_new_ebe3e0f6837f0879: function() {
            const ret = new Object();
            return ret;
        },
        __wbg_parse_1cc93481b0865939: function() { return handleError(function (arg0, arg1) {
            const ret = JSON.parse(getStringFromWasm0(arg0, arg1));
            return ret;
        }, arguments); },
        __wbg_set_6be42768c690e380: function(arg0, arg1, arg2) {
            arg0[arg1] = arg2;
        },
        __wbg_set_a80955eb93b145c6: function(arg0, arg1, arg2) {
            arg0[arg1 >>> 0] = arg2;
        },
        __wbindgen_cast_0000000000000001: function(arg0) {
            // Cast intrinsic for `F64 -> Externref`.
            const ret = arg0;
            return ret;
        },
        __wbindgen_cast_0000000000000002: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(U8)) -> NamedExternref("Uint8Array")`.
            const ret = getArrayU8FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_cast_0000000000000003: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
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
        "./cyoa_wasm_bg.js": import0,
    };
}

const WasmEngineFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmengine_free(ptr, 1));
const WasmStoryCatalogFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmstorycatalog_free(ptr, 1));

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

function getArrayJsValueFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    const mem = getDataViewMemory0();
    const result = [];
    for (let i = ptr; i < ptr + 4 * len; i += 4) {
        result.push(wasm.__wbindgen_externrefs.get(mem.getUint32(i, true)));
    }
    wasm.__externref_drop_slice(ptr, len);
    return result;
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

function getStringFromWasm0(ptr, len) {
    return decodeText(ptr >>> 0, len);
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

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
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

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (!module.ok) {
            throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
        }

        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = expectedResponseType(module.type);

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
        module_or_path = new URL('cyoa_wasm_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
