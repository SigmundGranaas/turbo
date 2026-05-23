/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // Match the Turbo brand neutrals used in the Flutter app.
        ink: {
          50: "#fafaf9",
          100: "#f5f5f4",
          200: "#e7e5e4",
          400: "#a8a29e",
          500: "#78716c",
          700: "#44403c",
          900: "#1c1917",
        },
        brand: {
          hiking: "#E53935",
          ski: "#1E88E5",
          forest: "#6D4C41",
          cycling: "#43A047",
        },
      },
    },
  },
  plugins: [],
};
