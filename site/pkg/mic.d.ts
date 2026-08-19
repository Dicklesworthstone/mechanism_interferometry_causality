/* tslint:disable */
/* eslint-disable */

/**
 * Audits Boolean design geometry for the supplied corner labels.
 */
export function design_audit(corners: string[], tolerance: number): string;

/**
 * Audits interaction aliasing for the supplied Boolean corner labels.
 */
export function interaction_aliasing(corners: string[], tolerance: number): string;

/**
 * Compares a serialized lens battery under the supplied tolerance.
 */
export function lens_battery(estimates_json: string, tolerance: number): string;

/**
 * Runs fail-closed manifest preflight and returns its JSON report.
 */
export function preflight(manifest_json: string, policy_json: string): string;

/**
 * Checks whether four declared sampling masses have product pooled odds.
 */
export function sampling_odds(probabilities: Float64Array, tolerance: number): string;

/**
 * Runs the deterministic built-in conformance simulations.
 */
export function simulate_all(): string;

/**
 * Enumerates complete square faces from supplied Boolean corners.
 */
export function square_faces(corners: string[]): string;

/**
 * Installs the browser panic hook when the WebAssembly module starts.
 */
export function start(): void;

/**
 * Validates one experiment manifest and returns a JSON result.
 */
export function validate_manifest(manifest_json: string): string;

/**
 * Returns the compiled MIC engine version.
 */
export function version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly design_audit: (a: number, b: number, c: number) => [number, number, number, number];
    readonly interaction_aliasing: (a: number, b: number, c: number) => [number, number, number, number];
    readonly lens_battery: (a: number, b: number, c: number) => [number, number, number, number];
    readonly preflight: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly sampling_odds: (a: number, b: number, c: number) => [number, number, number, number];
    readonly simulate_all: () => [number, number, number, number];
    readonly square_faces: (a: number, b: number) => [number, number, number, number];
    readonly validate_manifest: (a: number, b: number) => [number, number, number, number];
    readonly version: () => [number, number];
    readonly start: () => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __externref_table_alloc: () => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
