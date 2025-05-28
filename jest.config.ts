export default {
  displayName: 'bitsol-program',
  testEnvironment: 'node',
  roots: ['<rootDir>/tests'],
  testPathIgnorePatterns: [
    '/node_modules/',
    '/dist/',
    '/coverage/',
    '/.anchor/',
  ],
  transform: {
    '^.+\\.[tj]s$': ['ts-jest']
  },
  moduleFileExtensions: ['ts', 'js', 'html'],
  testTimeout: 45000,
};