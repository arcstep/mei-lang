/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./host-shell/app/src/**/*.rs",
    "./host-shell/app/assets/**/*.js",
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
