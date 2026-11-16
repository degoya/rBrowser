// Types of scripts/bundle.mjs for the freshness test.
export interface BundleSpec {
  entry: string;
  out: string;
}
export const BUNDLES: BundleSpec[];
export function bundle(spec: BundleSpec): Promise<string>;
