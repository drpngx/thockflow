/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["src/**/*.rs"],
  darkMode: 'media',
  theme: {
    extend: {
      fontFamily: {
        display: ["Major Mono Display", "sans-serif"],
        body: ["Raleway", "sans-serif"],
      },
    },
  },
  plugins: [],
};
