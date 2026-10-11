// Cross-platform explicit demo launcher, independent of a configured live .env.
process.env.BLOCKCONTEXT_MODE='demo';
await import('./main.js');
export {};
