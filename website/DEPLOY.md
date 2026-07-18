# Deploying the Falcon docs site to Cloudflare Workers

The site is a TanStack Start (React) app built with Vite. It deploys to
**Cloudflare Workers** as an SSR worker plus a static-assets binding, using the
official `@cloudflare/vite-plugin`. Wrangler is the deploy CLI.

- Worker name: `falcon-docs` (see `wrangler.toml`)
- SSR entry: `@tanstack/react-start/server-entry` (virtual module resolved by the plugin)
- Build output: `dist/server` (worker) + `dist/client` (static assets, auto-bound)

## Prerequisites

- Run inside the devenv shell so `bun` and `wrangler` are on PATH:
  ```sh
  devenv shell
  ```
  (Both are declared in `devenv.nix`.) Outside devenv, any Bun + `bunx wrangler`
  also works.

## Build & verify locally (no account needed)

```sh
cd website
bun run build          # produces dist/client + dist/server
bun run cf-dry-run     # build + `wrangler deploy --dry-run` — packages the worker offline, no login
bun run preview        # optional: serve the built worker locally via workerd
```

`cf-dry-run` is the safe smoke test: it bundles the worker and reads the assets
directory without contacting Cloudflare.

## One-time account setup — REQUIRES THE OWNER (Jacob)

These steps authenticate against and mutate a real Cloudflare account, so they
must be done by the account owner. **Do not run these unattended / from an
agent.**

1. **Authenticate wrangler** (opens a browser for OAuth, or use an API token):
   ```sh
   wrangler login
   ```
   Or set `CLOUDFLARE_API_TOKEN` (token with the *Workers Scripts: Edit* +
   *Account: Read* permissions) and `CLOUDFLARE_ACCOUNT_ID` in the environment
   for non-interactive/CI use.

2. **Confirm the account / worker name.** The first `wrangler deploy` creates the
   `falcon-docs` worker in the authenticated account. If the name collides or a
   different account is wanted, edit `name` in `wrangler.toml` first.

3. **Custom domain / DNS (owner).** The site's domain is
   **`dart-falcon.jacobdevelops.com`**, declared in `wrangler.toml` as a
   `custom_domain` route — `wrangler deploy` provisions the DNS record and
   certificate automatically, provided the `jacobdevelops.com` zone is on
   Cloudflare in the deploying account. (Until then the worker is also reachable
   at `falcon-docs.<subdomain>.workers.dev`.)

4. **Secrets, if/when added.** Runtime secrets go through
   `wrangler secret put <NAME>` (never committed). Local dev values go in
   `website/.dev.vars` (gitignored). There are none today — the site is static
   SSR with no backend bindings.

## Routine deploy (after one-time setup, run by the owner)

```sh
cd website
bun run deploy         # == bun run build && wrangler deploy
```

Wrangler uploads the worker and the `dist/client` assets to the `falcon-docs`
worker. Observability (request logs) is enabled in `wrangler.toml`.

## What is committed vs generated

- Committed: `wrangler.toml`, `vite.config.ts` (cloudflare plugin), deploy scripts
  in `package.json`.
- Generated / gitignored: `dist/`, `dist-check/`, `.wrangler/`, `.dev.vars`.
