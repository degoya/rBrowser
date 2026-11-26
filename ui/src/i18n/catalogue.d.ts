// The catalogue's two halves the Vite plugin serves (`cataloguePlugin.ts`, RB-099-20).

declare module 'virtual:rb-catalogue/shell' {
  const catalogue: Record<string, unknown>;
  export default catalogue;
}

declare module 'virtual:rb-catalogue/pages' {
  const catalogue: Record<string, unknown>;
  export default catalogue;
}
