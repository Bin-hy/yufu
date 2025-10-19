/** @type {import('tailwindcss').Config} */
export default {
  content: [
    './index.html',
    './src/**/*.{vue,js,ts,jsx,tsx}',
  ],
  theme: {
    extend: {
      colors: {
        primary: '#2563eb',
        secondary: '#4f46e5',
      },
      fontFamily: {
        inter: ['Inter', 'sans-serif'],
      },
    },
  },
  plugins: [
    require('daisyui'),
  ],
  daisyui: {
    themes: [
      {
        light: {
          primary: '#2563eb',
          secondary: '#4f46e5',
          accent: '#10b981',
          neutral: '#1f2937',
          "base-100": '#ffffff',
          "base-200": '#f3f4f6', // 稍深的灰色背景，而不是白色
          "base-300": '#e5e7eb',
          info: '#3b82f6',
          success: '#10b981',
          warning: '#f59e0b',
          error: '#ef4444',
        },
      },
    ],
  },
}
