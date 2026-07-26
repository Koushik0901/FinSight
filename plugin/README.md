# FinSight plugin

Connects Claude Code / Cowork to your self-hosted FinSight server and teaches it
how to use the tools well.

Shape: **skills + an MCP server reference** — the arrangement both
[Anthropic](https://claude.com/docs/connectors/building/what-to-build) and
[OpenAI](https://developers.openai.com/plugins/concepts/plugins) recommend when
workflow guidance should steer server-backed tools.

```
plugin/
├── .claude-plugin/plugin.json   manifest + the server-URL prompt
├── .mcp.json                    remote MCP reference (OAuth, no token to paste)
└── skills/
    ├── finsight-conventions/    always-on rules for reading the data correctly
    ├── financial-checkup/       "how am I doing"
    ├── spending-review/         "where is my money going / what changed"
    ├── debt-payoff/             payoff strategy, extra payments, sinking funds
    ├── affordability/           "can I afford X" and safe-to-spend
    └── apply-changes/           the draft → approve → execute write flow
```

The 48 tools come from the server, not from here — see
[`docs/self-hosting.md` §9](../docs/self-hosting.md). The skills add the
workflow judgement that tool descriptions alone can't carry: which tool answers
which question, which figures are easy to misread, and the approval discipline
for anything that writes.

## Install

```bash
claude plugin marketplace add Koushik0901/FinSight
```

```bash
claude plugin install finsight
```

You'll be asked for your **FinSight server URL** (the origin only — e.g.
`https://finsight.example.com` or `http://localhost:8674`; it's shown in
Settings → Connections). The plugin appends `/mcp` itself.

Authentication runs over OAuth on first use — the server registers the client,
you approve in FinSight's consent screen and pick an access level, and a token
is issued. Nothing to copy-paste. Revoke any time in Settings → Connections.

To connect **without** the plugin — Claude Desktop, claude.ai, ChatGPT, or bare
Claude Code — point the client straight at `https://<your-server>/mcp`. You get
the same tools, minus the skills. `docs/self-hosting.md` §9 covers which clients
can reach which URLs.

## Access levels

A **read-only** token limits the assistant to the 38 analysis tools. **Read and
write** adds the 10 that stage and apply proposals — still gated behind your
explicit approval in the conversation, and every proposal is visible in FinSight
itself. Start read-only.

## Editing the skills

`SKILL.md` files are plain markdown with YAML frontmatter. Edits take effect in
the current session; changes to `plugin.json` or `.mcp.json` need
`/reload-plugins`.

Keep them short. A skill's body stays in context once loaded, so every line is a
recurring cost — state the rule, not the rationale.
