/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./index.html", "./src/**/*.{vue,ts}"],
  theme: {
    extend: {
      transitionDuration: {
        page: "200ms",
        theme: "300ms"
      }
    }
  },
  plugins: []
};
