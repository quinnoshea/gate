module.exports = {
  darkMode: 'class',
  content: [
    "./index.html",
    "./src/**/*.{rs,html}",
    "../frontend-common/src/**/*.rs",
    "../chat-ui/src/**/*.rs",
  ],
  safelist: [
    // DEBUG: Force include common classes to test if scanning works
    'bg-gradient-to-br',
    'from-gray-900',
    'via-blue-900', 
    'to-purple-900',
    'backdrop-blur-lg',
    'bg-white/10',
    'rounded-2xl',
    'shadow-2xl',
    'text-3xl',
    'font-bold',
    'text-white',
    'border-white/20',
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