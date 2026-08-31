// GitHub Pages base.
// For https://koushik0901.github.io/FinSight/ the base must be "/FinSight/".
// For a custom domain (docs.finsight.app) or user-site root, change to "/".
const BASE = process.env.DOCS_BASE ?? "/FinSight/";

export default {
  title: "FinSight",
  description: "A quiet way to understand, plan, and master your money.",
  lang: "en-US",
  base: BASE,
  appearance: "dark",
  lastUpdated: true,
  cleanUrls: true,
  // Historical design/audit docs are not part of the published site.
  // They contain raw markdown that breaks the Vue markdown compiler.
  srcExclude: ["**/audits/**", "**/handoffs/**", "**/superpowers/**", "**/site/**", "**/qa/**", "**/design/**", "**/TODO.md", "**/mobile-ux-handoff.md", "**/agentic-finance-todo.md", "**/phase*.md"],
  sitemap: {
    hostname: "https://koushik0901.github.io/FinSight/",
  },
  head: [
    ["link", { rel: "icon", type: "image/svg+xml", href: `${BASE}logo.svg` }],
    ["link", { rel: "icon", type: "image/png", href: `${BASE}logo.svg` }],
    ["meta", { name: "theme-color", content: "#0A0F02" }],
    ["meta", { property: "og:type", content: "website" }],
    ["meta", { property: "og:title", content: "FinSight — Your AI-powered financial copilot" }],
    ["meta", { property: "og:description", content: "A quiet way to understand, plan, and master your money. Local-first, encrypted, self-hosted." }],
    ["meta", { property: "og:image", content: `${BASE}og.svg` }],
    ["meta", { name: "twitter:card", content: "summary_large_image" }],
    ["meta", { name: "twitter:title", content: "FinSight — Your AI-powered financial copilot" }],
    ["meta", { name: "twitter:description", content: "A quiet way to understand, plan, and master your money." }],
  ],
  markdown: {
    theme: {
      light: "github-light",
      dark: "github-dark",
    },
    lineNumbers: false,
  },
  themeConfig: {
    logo: "/logo.svg",
    siteTitle: "FinSight",
    outline: {
      level: [2, 3],
      label: "On this page",
    },
    search: {
      provider: "local",
    },
    nav: [
      { text: "Guide", link: "/getting-started/introduction" },
      { text: "Copilot", link: "/copilot/overview" },
      { text: "Developers", link: "/developers/architecture" },
      {
        text: "GitHub",
        link: "https://github.com/Koushik0901/FinSight",
      },
    ],
    sidebar: {
      "/getting-started/": [
        {
          text: "Getting Started",
          items: [
            { text: "Introduction", link: "/getting-started/introduction" },
            { text: "What is FinSight?", link: "/getting-started/what-is-finsight" },
            { text: "Installation", link: "/getting-started/installation" },
            { text: "First Launch", link: "/getting-started/first-launch" },
            { text: "Onboarding", link: "/getting-started/onboarding" },
            { text: "Importing Your Data", link: "/getting-started/importing-data" },
            { text: "Configuring AI", link: "/getting-started/configuring-ai" },
            { text: "Privacy & Local Data", link: "/getting-started/privacy" },
          ],
        },
      ],
      "/guide/": [
        {
          text: "Using FinSight",
          items: [
            { text: "Overview", link: "/guide/overview" },
            { text: "Today", link: "/guide/today" },
            { text: "Accounts", link: "/guide/accounts" },
            { text: "Transactions", link: "/guide/transactions" },
            { text: "Budget", link: "/guide/budget" },
            { text: "Categories", link: "/guide/categories" },
            { text: "Recurring", link: "/guide/recurring" },
            { text: "Goals", link: "/guide/goals" },
            { text: "Reports", link: "/guide/reports" },
            { text: "Insights", link: "/guide/insights" },
            { text: "Cash Flow", link: "/guide/cashflow" },
            { text: "Scenarios", link: "/guide/scenarios" },
            { text: "Recipes", link: "/guide/recipes" },
            { text: "Journey", link: "/guide/journey" },
          ],
        },
      ],
      "/copilot/": [
        {
          text: "Copilot",
          items: [
            { text: "Overview", link: "/copilot/overview" },
            { text: "How the Copilot Works", link: "/copilot/how-it-works" },
            { text: "Plans & Actions", link: "/copilot/plans" },
            { text: "Scenarios", link: "/copilot/scenarios" },
            { text: "Recipes", link: "/copilot/recipes" },
            { text: "Agent Memory", link: "/copilot/memory" },
            { text: "Privacy", link: "/copilot/privacy" },
          ],
        },
      ],
      "/framework/": [
        {
          text: "Financial Framework",
          items: [
            { text: "Overview", link: "/framework/overview" },
            { text: "Pay Yourself First", link: "/framework/pay-yourself-first" },
            { text: "Conscious Spending", link: "/framework/conscious-spending" },
            { text: "Emergency Fund", link: "/framework/emergency-fund" },
            { text: "Debt Snowball", link: "/framework/debt-snowball" },
            { text: "Compound Growth", link: "/framework/compound-growth" },
            { text: "Financial Journey", link: "/framework/journey" },
          ],
        },
      ],
      "/automation/": [
        {
          text: "Automation",
          items: [
            { text: "Overview", link: "/automation/overview" },
            { text: "Rules", link: "/automation/rules" },
            { text: "Categorization", link: "/automation/categorization" },
            { text: "Recipes", link: "/automation/recipes" },
          ],
        },
      ],
      "/configuration/": [
        {
          text: "Configuration",
          items: [
            { text: "Settings", link: "/configuration/settings" },
            { text: "Ollama (Local)", link: "/configuration/ollama" },
            { text: "OpenAI-Compatible", link: "/configuration/openai-compatible" },
            { text: "Anthropic", link: "/configuration/anthropic" },
            { text: "Data Storage", link: "/configuration/data-storage" },
          ],
        },
      ],
      "/developers/": [
        {
          text: "Developer Guide",
          items: [
            { text: "Architecture", link: "/developers/architecture" },
            { text: "Development Setup", link: "/developers/setup" },
            { text: "Repository Structure", link: "/developers/structure" },
            { text: "API & RPC", link: "/developers/api" },
            { text: "Frontend", link: "/developers/frontend" },
            { text: "Rust Crates", link: "/developers/crates" },
            { text: "OpenAPI Bindings", link: "/developers/bindings" },
            { text: "Testing", link: "/developers/testing" },
            { text: "CSS & Design", link: "/developers/css" },
            { text: "Contributing", link: "/developers/contributing" },
          ],
        },
      ],
      "/help/": [
        {
          text: "Help",
          items: [
            { text: "FAQ", link: "/help/faq" },
            { text: "Troubleshooting", link: "/help/troubleshooting" },
            { text: "Security & Privacy", link: "/help/security" },
            { text: "Self-Hosting", link: "/help/self-hosting" },
            { text: "Changelog", link: "/help/changelog" },
          ],
        },
      ],
    },
    socialLinks: [
      { icon: "github", link: "https://github.com/Koushik0901/FinSight" },
    ],
    editLink: {
      pattern: "https://github.com/Koushik0901/FinSight/edit/main/docs/:path",
      text: "Edit this page on GitHub",
    },
    lastUpdated: {
      text: "Last updated",
    },
    footer: {
      message: "Local-first. Encrypted. Yours.",
      copyright: "© FinSight — MIT Licensed",
    },
    docFooter: {
      prev: "Previous",
      next: "Next",
    },
  },
  vite: {
    server: {
      port: 5174,
    },
  },
};
