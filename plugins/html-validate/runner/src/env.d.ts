// Vite's `?raw` imports in the tests: the file's text.
declare module '*?raw' {
  const text: string;
  export default text;
}
