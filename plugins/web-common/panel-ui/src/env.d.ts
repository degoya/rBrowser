// Single-file components are type-checked by the Vite build and their tests, not by tsc.
declare module '*.vue' {
  import type { DefineComponent } from 'vue';
  const component: DefineComponent<object, object, unknown>;
  export default component;
}

// A panel's stylesheet is imported for its side effect; Vite compiles it.
declare module '*.css';
