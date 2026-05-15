/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./app/src/**/*.rs",
    "./app/assets/**/*.js",
  ],
  corePlugins: {
    preflight: false,
  },
  theme: {
    extend: {
      colors: {
        mei: {
          panel: "#0f172a",
          text: "#e2e8f0",
        },
      },
    },
  },
};
