/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        emo: {
          cyan: '#00d9ff',
          orange: '#ff6b35',
          dark: '#1a1a1a',
        }
      },
    },
  },
  plugins: [],
}
