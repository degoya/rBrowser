// The project source of the Tailwind v3 fixture (RB-030-07): the default screens plus one the
// start page does not use yet, so only the static source finds 1920.
/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ['./index.html'],
  theme: {
    extend: {
      colors: { brand: '#0f766e' },
      screens: { '3xl': '1920px' },
    },
  },
  plugins: [],
};
