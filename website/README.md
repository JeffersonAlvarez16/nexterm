# nexterm.dev — landing page

Static landing page for NexTerm, built with SvelteKit + adapter-static.

## Develop

```sh
pnpm install
pnpm dev
```

## Build

```sh
pnpm build   # outputs static site to build/
```

## Deploy

The output in `build/` is plain static files — host it anywhere with HTTPS
(mandatory for `.dev` domains, the whole TLD is HSTS-preloaded):

- **Vercel**: import the repo, set Root Directory to `website/`. Framework
  preset SvelteKit is detected automatically.
- **Cloudflare Pages / Netlify**: build command `pnpm build`, output `build/`.
- **GitHub Pages**: publish the `build/` folder via an action.

Then point the `.dev` domain's DNS at the host and enforce HTTPS.
