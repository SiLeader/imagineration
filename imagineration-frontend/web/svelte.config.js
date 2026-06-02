import adapter from '@sveltejs/adapter-static';

const config = {
  kit: {
    adapter: adapter({
      pages: '../dist',
      assets: '../dist',
      fallback: 'index.html',
      precompress: false,
      strict: true
    }),
    paths: {
      relative: true
    }
  }
};

export default config;
