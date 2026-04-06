/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        tg: {
          bg: 'var(--tg-bg)',
          text: 'var(--tg-text)',
          hint: 'var(--tg-hint)',
          button: 'var(--tg-button)',
          'button-text': 'var(--tg-button-text)',
          danger: 'var(--tg-danger)',
          success: 'var(--tg-success)',
          warning: 'var(--tg-warning)',
          surface: 'var(--tg-surface)',
        },
      },
    },
  },
  plugins: [],
};
