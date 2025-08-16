module.exports = {
  darkMode: 'class',
  content: [
    "./index.html",
    "./src/**/*.{rs,html}",
    "../frontend-common/src/**/*.rs",
    "../chat-ui/src/**/*.rs",
  ],
  theme: {
    extend: {
      animation: {
        'spin': 'spin 1s linear infinite',
        'pulse-dot': 'pulse-dot 1.4s ease-in-out infinite',
      },
      keyframes: {
        'pulse-dot': {
          '0%, 80%, 100%': {
            opacity: '0.3',
            transform: 'scale(0.8)',
          },
          '40%': {
            opacity: '1',
            transform: 'scale(1)',
          },
        },
      },
    },
  },
  plugins: [],
}