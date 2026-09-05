import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'ArcadeJanitor',
  description: 'A focused CLI and MCP server for managing MAME ROM collections.',
  base: '/arcade-janitor/',
  outDir: 'dist',
  appearance: 'auto',

  themeConfig: {
    logo: '/images/arcadejanitor.png',
    nav: [
      { text: 'Installation', link: '/installation' },
      { text: 'CLI Reference', link: '/cli' },
      { text: 'MCP Server', link: '/mcp' },
      { text: 'Development', link: '/development' },
    ],
    sidebar: [
      {
        text: 'Getting Started',
        items: [
          { text: 'Overview', link: '/' },
          { text: 'Installation', link: '/installation' },
        ],
      },
      {
        text: 'Using ArcadeJanitor',
        items: [{ text: 'CLI Reference', link: '/cli' }],
      },
      {
        text: 'MCP Server',
        items: [{ text: 'Setup and Configuration', link: '/mcp' }],
      },
      {
        text: 'Contributing',
        items: [{ text: 'Development Guide', link: '/development' }],
      },
    ],
    socialLinks: [
      { icon: 'github', link: 'https://github.com/PlagueHO/arcade-janitor' },
    ],
    footer: {
      message: 'Released under the MIT License.',
    },
  },
})
