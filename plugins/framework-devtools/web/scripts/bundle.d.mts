// Types of scripts/bundle.mjs for the freshness test.
export interface BundleSpec {
  entry: string;
  out: string;
  budget: number;
}
export const BUNDLES: BundleSpec[];
export function bundle(spec: BundleSpec): Promise<string>;
export function thirdPartyLicences(): Promise<string>;
