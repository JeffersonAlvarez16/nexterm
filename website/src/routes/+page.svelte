<script>
	import Terminal from '$lib/Terminal.svelte';

	const RELEASES = 'https://github.com/JeffersonAlvarez16/nexterm/releases/latest';
	const REPO = 'https://github.com/JeffersonAlvarez16/nexterm';

	/** Scroll-reveal action: adds .in when the element enters the viewport. */
	function reveal(node) {
		const io = new IntersectionObserver(
			(entries) => {
				for (const e of entries) {
					if (e.isIntersecting) {
						node.classList.add('in');
						io.disconnect();
					}
				}
			},
			{ threshold: 0.18 }
		);
		io.observe(node);
		return { destroy: () => io.disconnect() };
	}

	const ribbon = [
		'SSH', 'AES-256-GCM', 'Argon2id', 'QEMU', 'LXC', 'snapshots', 'auto-lock',
		'signed updates', 'notarized', 'Bitwarden import', 'zeroized memory', 'Rust core'
	];

	const features = [
		{
			title: 'ssh — sessions that feel native',
			heading: 'A terminal, not a tab',
			body: 'NexTerm is a native Tauri app with a Rust core — no Electron weight. Pin your servers, jump between sessions, and keep working in English or Spanish.',
			points: ['Native macOS, Windows & Linux', 'Multi-session workspace', 'Single-instance, instant resume']
		},
		{
			title: 'vault — secrets, sealed',
			heading: 'A vault where you type',
			body: 'Credentials live next to your sessions, encrypted with AES-256-GCM behind an Argon2id master password. It locks itself after 60 seconds of inactivity.',
			points: ['Bitwarden-compatible import & export', 'Re-auth gate before any plaintext export', 'Plaintext buffers zeroized in memory']
		},
		{
			title: 'proxmox — your homelab, one keystroke away',
			heading: 'Drive Proxmox from the terminal',
			body: 'First-class control of LXC containers and QEMU virtual machines over SSH: list, start, stop, reboot and snapshot — with injection-safe command building.',
			points: ['LXC (pct) + QEMU (qm) side by side', 'Create, roll back & delete snapshots', 'Validated VMIDs and snapshot names']
		}
	];

	const security = [
		['Code-signed & notarized', 'macOS builds are Developer ID signed and notarized by Apple; Windows and Linux ship checksummed artifacts.'],
		['Signed auto-updates', 'Every update is verified against a minisign public key embedded in the app before it installs. No valid signature, no update.'],
		['Encryption that costs something', 'AES-256-GCM for data at rest, Argon2id for key derivation — chosen to make brute force expensive.'],
		['Hardened import/export', 'Record-aware CSV parsing, formula-injection neutralization on export, and rollback on partial import failure.'],
		['Memory hygiene', 'Plaintext secrets are zeroized after use instead of waiting for the garbage collector that Rust does not have anyway.']
	];

	const downloads = [
		{
			os: 'macOS',
			detail: 'Universal — Apple Silicon + Intel',
			art: 'one .dmg, signed & notarized',
			glyph: 'darwin'
		},
		{
			os: 'Windows',
			detail: 'x64 — installer or MSI',
			art: '.exe · .msi',
			glyph: 'win32'
		},
		{
			os: 'Linux',
			detail: 'x64 — pick your format',
			art: '.deb · .rpm · .AppImage',
			glyph: 'linux'
		}
	];

	let copied = $state(false);
	const brewCmd = 'brew tap JeffersonAlvarez16/tap && brew install --cask nexterm';

	async function copyBrew() {
		try {
			await navigator.clipboard.writeText(brewCmd);
			copied = true;
			setTimeout(() => (copied = false), 1800);
		} catch {
			/* clipboard unavailable — leave the command selectable */
		}
	}
</script>

<!-- ── Nav ──────────────────────────────────────────────────────────────── -->
<header class="nav wrap">
	<a class="brand" href="/">
		<img src="/icon.svg" alt="" width="30" height="30" />
		<span>nexterm<em>.dev</em></span>
	</a>
	<nav>
		<a href="#features">Features</a>
		<a href="#security">Security</a>
		<a href="#download">Download</a>
		<a class="gh" href={REPO} target="_blank" rel="noreferrer">GitHub ↗</a>
	</nav>
</header>

<!-- ── Hero ─────────────────────────────────────────────────────────────── -->
<section class="hero wrap">
	<div class="hero-copy">
		<p class="kicker hero-stagger" style="--i: 0">v0.4.2 — signed · notarized · auto-updating</p>
		<h1 class="hero-stagger" style="--i: 1">
			Your servers.<br />
			Your secrets.<br />
			<span class="aurora-text">One terminal.</span>
		</h1>
		<p class="lede hero-stagger" style="--i: 2">
			NexTerm is a native SSH client with an encrypted password vault and first-class
			Proxmox control — built in Rust, at home on macOS, Windows and Linux.
		</p>
		<div class="cta hero-stagger" style="--i: 3">
			<a class="btn btn-primary" href={RELEASES} target="_blank" rel="noreferrer">
				▼ Download free
			</a>
			<a class="btn btn-ghost" href="#features">See what it does</a>
		</div>
		<p class="trust hero-stagger" style="--i: 4">
			Open source · no account · no telemetry
		</p>
	</div>
	<div class="hero-term hero-stagger" style="--i: 2">
		<Terminal />
	</div>
</section>

<!-- ── Ribbon ───────────────────────────────────────────────────────────── -->
<div class="ribbon" aria-hidden="true">
	<div class="ribbon-track">
		{#each [0, 1] as half}
			<div class="ribbon-half" aria-hidden={half === 1}>
				{#each ribbon as word}
					<span>{word}</span><i>◆</i>
				{/each}
			</div>
		{/each}
	</div>
</div>

<!-- ── Features ─────────────────────────────────────────────────────────── -->
<section id="features" class="features wrap">
	<p class="kicker" use:reveal>What ships in the box</p>
	<h2 use:reveal>Three tools that usually<br />live in three apps.</h2>
	<div class="feature-grid">
		{#each features as f, i}
			<article class="feature reveal" use:reveal style="transition-delay: {i * 90}ms">
				<div class="feature-bar">
					<span class="dot"></span><span class="dot"></span><span class="dot"></span>
					<span class="feature-title">{f.title}</span>
				</div>
				<div class="feature-body">
					<h3>{f.heading}</h3>
					<p>{f.body}</p>
					<ul>
						{#each f.points as p}
							<li><span class="tick">❯</span>{p}</li>
						{/each}
					</ul>
				</div>
			</article>
		{/each}
	</div>
</section>

<!-- ── Security ─────────────────────────────────────────────────────────── -->
<section id="security" class="security">
	<div class="wrap security-grid">
		<div class="security-lead reveal" use:reveal>
			<p class="kicker">Threat model included</p>
			<h2>Paranoid<br />by design.</h2>
			<p class="lede">
				A terminal that stores credentials has one job above all others:
				don't leak them. Every release is built like it will be attacked,
				because eventually it will be.
			</p>
		</div>
		<ul class="security-list">
			{#each security as [t, d], i}
				<li class="reveal" use:reveal style="transition-delay: {i * 70}ms">
					<span class="sec-index">0{i + 1}</span>
					<div>
						<strong>{t}</strong>
						<p>{d}</p>
					</div>
				</li>
			{/each}
		</ul>
	</div>
</section>

<!-- ── Download ─────────────────────────────────────────────────────────── -->
<section id="download" class="download wrap">
	<p class="kicker" use:reveal>Get NexTerm</p>
	<h2 use:reveal>Pick your platform.<br />Updates handle themselves.</h2>
	<div class="dl-grid">
		{#each downloads as d, i}
			<a
				class="dl-card reveal"
				use:reveal
				style="transition-delay: {i * 90}ms"
				href={RELEASES}
				target="_blank"
				rel="noreferrer"
			>
				<span class="dl-glyph"><i>❯</i> {d.glyph}</span>
				<strong>{d.os}</strong>
				<span class="dl-detail">{d.detail}</span>
				<span class="dl-art">{d.art}</span>
				<span class="dl-go">download ❯</span>
			</a>
		{/each}
	</div>
	<div class="brew reveal" use:reveal>
		<span class="brew-label">or on macOS:</span>
		<code>{brewCmd}</code>
		<button onclick={copyBrew}>{copied ? '✓ copied' : 'copy'}</button>
	</div>
</section>

<!-- ── Footer ───────────────────────────────────────────────────────────── -->
<footer class="footer">
	<div class="wrap footer-grid">
		<a class="brand" href="/">
			<img src="/icon.svg" alt="" width="26" height="26" />
			<span>nexterm<em>.dev</em></span>
		</a>
		<p>Built with Rust + Tauri. Open source on <a href={REPO} target="_blank" rel="noreferrer">GitHub</a>.</p>
		<p class="foot-mono">❯ exit<span class="foot-cursor"></span></p>
	</div>
</footer>

<style>
	/* ── Nav ── */
	.nav {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-block: 1.4rem;
	}

	.brand {
		display: inline-flex;
		align-items: center;
		gap: 0.6rem;
		font-family: var(--mono);
		font-weight: 700;
		font-size: 1.02rem;
		text-decoration: none;
		letter-spacing: -0.02em;
	}

	.brand em {
		font-style: normal;
		color: var(--blue);
	}

	.nav nav {
		display: flex;
		gap: 1.8rem;
		font-family: var(--mono);
		font-size: 0.85rem;
	}

	.nav nav a {
		text-decoration: none;
		color: var(--ink-dim);
		transition: color 0.2s;
	}

	.nav nav a:hover { color: var(--cyan); }
	.nav nav a.gh { color: var(--ink); }

	/* ── Hero ── */
	.hero {
		display: grid;
		grid-template-columns: 1.05fr 0.95fr;
		gap: 3.5rem;
		align-items: center;
		padding-block: 5.5rem 6rem;
	}

	.hero h1 {
		font-size: clamp(2.6rem, 5.4vw, 4.4rem);
		font-weight: 750;
		line-height: 1.02;
		letter-spacing: -0.03em;
		margin-block: 1.1rem 1.4rem;
		font-variation-settings: 'opsz' 96;
	}

	.lede {
		color: var(--ink-dim);
		font-size: 1.08rem;
		max-width: 34rem;
	}

	.cta {
		display: flex;
		gap: 0.9rem;
		margin-top: 2rem;
		flex-wrap: wrap;
	}

	.trust {
		margin-top: 1.3rem;
		font-family: var(--mono);
		font-size: 0.78rem;
		color: var(--ink-faint);
		letter-spacing: 0.05em;
	}

	.hero-term {
		transform: rotate(1.2deg);
		transition: transform 0.5s var(--ease-out);
	}

	.hero-term:hover { transform: rotate(0deg) scale(1.01); }

	.hero-stagger {
		animation: rise 0.85s var(--ease-out) both;
		animation-delay: calc(var(--i) * 110ms);
	}

	@keyframes rise {
		from { opacity: 0; transform: translateY(24px); }
	}

	/* ── Ribbon ── */
	.ribbon {
		border-block: 1px solid rgba(99, 130, 199, 0.14);
		background: rgba(10, 15, 30, 0.5);
		overflow: hidden;
		padding-block: 0.8rem;
		mask-image: linear-gradient(90deg, transparent, #000 8%, #000 92%, transparent);
	}

	.ribbon-track {
		display: flex;
		width: max-content;
		animation: slide 36s linear infinite;
	}

	.ribbon-half {
		display: flex;
		align-items: center;
		font-family: var(--mono);
		font-size: 0.8rem;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--ink-faint);
		white-space: nowrap;
	}

	.ribbon-half span { padding-inline: 1.4rem; }

	.ribbon-half i {
		font-style: normal;
		font-size: 0.5rem;
		color: var(--indigo);
	}

	@keyframes slide {
		to { transform: translateX(-50%); }
	}

	/* ── Sections shared ── */
	section h2 {
		font-size: clamp(1.9rem, 3.6vw, 2.9rem);
		font-weight: 720;
		line-height: 1.08;
		letter-spacing: -0.025em;
		margin-block: 0.7rem 2.6rem;
	}

	/* ── Features ── */
	.features { padding-block: 6.5rem 4rem; }

	.feature-grid {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 1.4rem;
	}

	.feature {
		background: linear-gradient(165deg, rgba(16, 23, 41, 0.85), rgba(8, 12, 22, 0.9));
		border: 1px solid var(--panel-edge);
		border-radius: var(--radius);
		overflow: hidden;
		transition: transform 0.35s var(--ease-out), border-color 0.35s, box-shadow 0.35s;
	}

	.feature:hover {
		transform: translateY(-6px);
		border-color: rgba(56, 189, 246, 0.4);
		box-shadow: 0 24px 60px rgba(2, 6, 16, 0.7), 0 0 50px rgba(56, 189, 246, 0.1);
	}

	.feature-bar {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 0.65rem 0.95rem;
		border-bottom: 1px solid rgba(99, 130, 199, 0.12);
		background: rgba(10, 15, 30, 0.6);
	}

	.feature-bar .dot {
		width: 9px;
		height: 9px;
		border-radius: 50%;
		background: rgba(120, 140, 180, 0.25);
	}

	.feature-title {
		margin-left: 0.55rem;
		font-family: var(--mono);
		font-size: 0.7rem;
		color: var(--ink-faint);
		letter-spacing: 0.04em;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.feature-body { padding: 1.5rem 1.5rem 1.7rem; }

	.feature-body h3 {
		font-size: 1.28rem;
		font-weight: 700;
		letter-spacing: -0.02em;
		margin-bottom: 0.7rem;
	}

	.feature-body > p {
		color: var(--ink-dim);
		font-size: 0.95rem;
		margin-bottom: 1.2rem;
	}

	.feature-body ul {
		list-style: none;
		display: grid;
		gap: 0.55rem;
	}

	.feature-body li {
		display: flex;
		gap: 0.6rem;
		font-family: var(--mono);
		font-size: 0.8rem;
		color: var(--ink-dim);
	}

	.tick { color: var(--cyan); font-weight: 700; }

	/* ── Security ── */
	.security {
		padding-block: 5.5rem;
		background:
			radial-gradient(ellipse 50rem 26rem at 18% 40%, rgba(99, 102, 241, 0.09), transparent 65%),
			linear-gradient(180deg, transparent, rgba(8, 12, 22, 0.7) 18%, rgba(8, 12, 22, 0.7) 82%, transparent);
	}

	.security-grid {
		display: grid;
		grid-template-columns: 0.9fr 1.1fr;
		gap: 4rem;
		align-items: start;
	}

	.security-lead { position: sticky; top: 4rem; }

	.security-lead h2 { margin-bottom: 1.2rem; }

	.security-list {
		list-style: none;
		display: grid;
		gap: 0;
	}

	.security-list li {
		display: flex;
		gap: 1.4rem;
		padding: 1.35rem 0.4rem;
		border-bottom: 1px dashed rgba(99, 130, 199, 0.18);
	}

	.security-list li:first-child { border-top: 1px dashed rgba(99, 130, 199, 0.18); }

	.sec-index {
		font-family: var(--mono);
		font-size: 0.78rem;
		color: var(--blue);
		padding-top: 0.25rem;
	}

	.security-list strong {
		display: block;
		font-size: 1.05rem;
		letter-spacing: -0.01em;
		margin-bottom: 0.25rem;
	}

	.security-list p {
		color: var(--ink-dim);
		font-size: 0.92rem;
	}

	/* ── Download ── */
	.download { padding-block: 6.5rem 5rem; }

	.dl-grid {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 1.4rem;
	}

	.dl-card {
		display: grid;
		gap: 0.3rem;
		padding: 1.8rem 1.7rem;
		text-decoration: none;
		background: linear-gradient(165deg, rgba(16, 23, 41, 0.85), rgba(8, 12, 22, 0.9));
		border: 1px solid var(--panel-edge);
		border-radius: var(--radius);
		transition: transform 0.35s var(--ease-out), border-color 0.35s, box-shadow 0.35s;
	}

	.dl-card:hover {
		transform: translateY(-6px);
		border-color: rgba(103, 232, 249, 0.5);
		box-shadow: 0 24px 60px rgba(2, 6, 16, 0.7), 0 0 50px rgba(103, 232, 249, 0.12);
	}

	.dl-glyph {
		font-family: var(--mono);
		font-size: 0.8rem;
		color: var(--ink-faint);
		letter-spacing: 0.1em;
		margin-bottom: 0.6rem;
	}

	.dl-glyph i {
		font-style: normal;
		color: var(--blue);
		font-weight: 700;
	}

	.dl-card strong {
		font-size: 1.35rem;
		font-weight: 720;
		letter-spacing: -0.02em;
	}

	.dl-detail { color: var(--ink-dim); font-size: 0.92rem; }

	.dl-art {
		font-family: var(--mono);
		font-size: 0.76rem;
		color: var(--ink-faint);
		margin-top: 0.5rem;
	}

	.dl-go {
		font-family: var(--mono);
		font-size: 0.82rem;
		color: var(--cyan);
		margin-top: 1.1rem;
		transition: letter-spacing 0.3s var(--ease-out);
	}

	.dl-card:hover .dl-go { letter-spacing: 0.08em; }

	.brew {
		display: flex;
		align-items: center;
		gap: 1rem;
		margin-top: 2.2rem;
		padding: 0.9rem 1.2rem;
		border: 1px dashed rgba(99, 130, 199, 0.3);
		border-radius: 10px;
		font-family: var(--mono);
		flex-wrap: wrap;
	}

	.brew-label { color: var(--ink-faint); font-size: 0.82rem; }

	.brew code {
		color: var(--cyan);
		font-size: 0.84rem;
		overflow-wrap: anywhere;
	}

	.brew button {
		margin-left: auto;
		font-family: var(--mono);
		font-size: 0.78rem;
		color: var(--ink);
		background: rgba(56, 189, 246, 0.12);
		border: 1px solid rgba(56, 189, 246, 0.35);
		border-radius: 7px;
		padding: 0.4rem 0.9rem;
		cursor: pointer;
		transition: background 0.2s;
	}

	.brew button:hover { background: rgba(56, 189, 246, 0.25); }

	/* ── Footer ── */
	.footer {
		border-top: 1px solid rgba(99, 130, 199, 0.14);
		padding-block: 2.4rem;
		background: rgba(7, 10, 19, 0.8);
	}

	.footer-grid {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1.5rem;
		flex-wrap: wrap;
	}

	.footer p {
		color: var(--ink-faint);
		font-size: 0.88rem;
	}

	.footer a { color: var(--ink-dim); }

	.foot-mono {
		font-family: var(--mono);
		color: var(--ink-faint);
	}

	.foot-cursor {
		display: inline-block;
		width: 0.55em;
		height: 1.05em;
		margin-left: 4px;
		background: var(--blue);
		vertical-align: text-bottom;
		animation: blink 1.05s steps(1) infinite;
	}

	@keyframes blink { 50% { opacity: 0; } }

	/* ── Responsive ── */
	@media (max-width: 940px) {
		.hero {
			grid-template-columns: 1fr;
			padding-block: 3.5rem 4rem;
			gap: 3rem;
		}

		.hero-term { transform: none; }

		.feature-grid, .dl-grid { grid-template-columns: 1fr; }

		.security-grid {
			grid-template-columns: 1fr;
			gap: 2.5rem;
		}

		.security-lead { position: static; }

		.nav nav { gap: 1.1rem; font-size: 0.78rem; }
	}
</style>
