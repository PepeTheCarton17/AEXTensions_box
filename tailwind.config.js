/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx,html}",
  ],
  theme: {
    extend: {
      colors: {
        brand: {
          50: '#f5f3ff',
          100: '#ede9fe',
          200: '#ddd6fe',
          300: '#c4b5fd',
          400: '#a78bfa',
          500: '#8b5cf6',
          600: '#7c3aed',
          700: '#6d28d9',
          800: '#5b21b6',
          900: '#4c1d95',
          950: '#2e1065',
        },
        surface: {
          700: '#2c3149',
          800: '#202436',
          850: '#1a1d2c',
          900: '#141622',
          950: '#0d0f17',
          dark: '#0f111a',
          card: '#181b26',
          border: '#262a3b',
          input: '#12141d',
          hover: '#1f2433'
        }
      }
    },
  },
  plugins: [],
}
