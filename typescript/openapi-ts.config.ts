import { defineConfig } from '@hey-api/openapi-ts';

export default defineConfig({
  input: '../generated/filtered-openapi.json',
  output: 'src/types',
  plugins: ['@hey-api/typescript'],
});
