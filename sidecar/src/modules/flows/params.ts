// The parameters of `flows.run`, checked before a worker starts: rBrowser builds them, but the
// worker's permissions are derived from `projectRoot` and `outDir`, so both must be absolute
// and the run id a plain name.

import { isAbsolute } from 'node:path';

import { ErrorCode, RpcError } from '../../rpc.ts';
import type { JobView, RunJob } from './result.ts';

export interface RunParams extends RunJob {
  /** Readable by the worker (fixtures); the flow's own text comes in `flow.source`. */
  projectRoot: string;
}

/** The most views one run takes. */
export const MAX_VIEWS = 32;
/** The longest flow source taken. */
export const MAX_SOURCE_BYTES = 1024 * 1024;

const RUN_ID = /^[A-Za-z0-9][A-Za-z0-9-]{0,63}$/;

function fail(message: string): never {
  throw new RpcError(ErrorCode.InvalidParams, message);
}

function record(value: unknown, what: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    fail(`${what} must be an object`);
  }
  return value as Record<string, unknown>;
}

function text(value: unknown, what: string): string {
  if (typeof value !== 'string') {
    fail(`${what} must be a string`);
  }
  return value;
}

function count(value: unknown, what: string, min: number, max: number): number {
  if (typeof value !== 'number' || !Number.isInteger(value) || value < min || value > max) {
    fail(`${what} must be a whole number from ${min} to ${max}`);
  }
  return value;
}

function absolute(value: unknown, what: string): string {
  const path = text(value, what);
  if (!isAbsolute(path) && !/^[A-Za-z]:[\\/]|^\\\\/.test(path)) {
    fail(`${what} must be an absolute path`);
  }
  return path;
}

function view(value: unknown, index: number): JobView {
  const v = record(value, `views[${index}]`);
  const endpoint = record(v.endpoint, `views[${index}].endpoint`);
  const headers = record(endpoint.headers ?? {}, `views[${index}].endpoint.headers`);
  return {
    id: count(v.id, `views[${index}].id`, 0, 2 ** 32 - 1),
    label: text(v.label, `views[${index}].label`),
    width: count(v.width, `views[${index}].width`, 0, 100_000),
    height: count(v.height, `views[${index}].height`, 0, 100_000),
    engine: text(v.engine, `views[${index}].engine`),
    breakpoint: v.breakpoint === null || v.breakpoint === undefined ? null : text(v.breakpoint, `views[${index}].breakpoint`),
    endpoint: {
      url: text(endpoint.url, `views[${index}].endpoint.url`),
      headers: Object.fromEntries(Object.entries(headers).map(([k, h]) => [k, text(h, `header ${k}`)])),
    },
  };
}

export function checkParams(params: unknown): RunParams {
  const p = record(params, 'params');
  const runId = text(p.runId, 'runId');
  if (!RUN_ID.test(runId)) {
    fail('runId must be letters, digits and dashes');
  }
  const flow = record(p.flow, 'flow');
  const source = text(flow.source, 'flow.source');
  if (Buffer.byteLength(source) > MAX_SOURCE_BYTES) {
    fail(`flow.source is longer than ${MAX_SOURCE_BYTES} bytes`);
  }
  if (!Array.isArray(p.views) || p.views.length === 0 || p.views.length > MAX_VIEWS) {
    fail(`views must list 1 to ${MAX_VIEWS} views`);
  }
  return {
    runId,
    flow: { name: text(flow.name, 'flow.name'), source, file: text(flow.file, 'flow.file') },
    views: p.views.map(view),
    env: p.env === null || p.env === undefined ? null : text(p.env, 'env'),
    timeoutMs: count(p.timeoutMs, 'timeoutMs', 1, 3_600_000),
    parallel: count(p.parallel ?? 0, 'parallel', 0, MAX_VIEWS),
    screenshotOnFailure: p.screenshotOnFailure !== false,
    outDir: absolute(p.outDir, 'outDir'),
    projectRoot: absolute(p.projectRoot, 'projectRoot'),
  };
}
