<script>
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
			{ threshold: 0.15 }
		);
		io.observe(node);
		return { destroy: () => io.disconnect() };
	}

	/* Every claim below is implemented in the app — see src/ and src-tauri/src/. */
	const pillars = [
		{
			tag: 'terminal',
			heading: 'A terminal that keeps up',
			body: 'xterm.js on WebGL, tabs and split panes per session, in-terminal search, command snippets with variables, and input broadcast to every pane when you mean it.',
			points: ['Tabs + resizable split panes', 'Snippets with {{variables}}', 'Broadcast input across panes']
		},
		{
			tag: 'sftp',
			heading: 'Files without leaving',
			body: 'Dual-pane local/remote browser with drag-and-drop both ways, recursive folder downloads with progress, conflict resolution, and remote file editing in-app.',
			points: ['Drag & drop transfers', 'Recursive downloads', 'View & edit remote files']
		},
		{
			tag: 'vault',
			heading: 'Two vaults, zero plaintext',
			body: 'SSH credentials and personal passwords live in separate stores, each AES-256-GCM encrypted behind an Argon2id master password. Auto-lock after idle, re-auth to reveal.',
			points: ['AES-256-GCM + Argon2id', 'Auto-lock & suspend-lock', 'Bitwarden import / export']
		},
		{
			tag: 'tunnels',
			heading: 'Tunnels you can see',
			body: 'Local and remote port forwards plus a full SOCKS5 dynamic proxy, saved per profile, each showing live state, connection count and bytes in/out.',
			points: ['-L, -R and SOCKS5 (-D)', 'Saved with each profile', 'Live traffic counters']
		},
		{
			tag: 'panels',
			heading: 'Your fleet on one screen',
			body: 'Remote monitoring with CPU, memory, disk and network sparklines. Docker containers: start, stop, logs. Proxmox LXC and QEMU: lifecycle and snapshots.',
			points: ['Monitoring sparklines', 'Docker over SSH', 'Proxmox LXC + QEMU snapshots']
		},
		{
			tag: 'identity',
			heading: 'Keys, done right',
			body: 'Password, key, agent and keyboard-interactive auth. Generate Ed25519, RSA or ECDSA keys in-app. ProxyJump bastion support and OpenSSH known_hosts verification.',
			points: ['ssh-agent & MFA support', 'In-app keygen (Ed25519…)', 'Host-key trust on first use']
		}
	];

	const shots = [
		{ id: 'terminal', label: 'Terminal', src: '/shots/terminal.png', alt: 'VaulTerm terminal session with Docker output and the profile sidebar' },
		{ id: 'sftp', label: 'SFTP', src: '/shots/sftp.png', alt: 'Dual-pane SFTP browser with local and remote files' },
		{ id: 'launchpad', label: 'Launchpad', src: '/shots/launchpad.png', alt: 'Launchpad with saved profiles and recent connections' },
		{ id: 'profile', label: 'Profiles', src: '/shots/profile.png', alt: 'Connection profile editor' }
	];

	let activeShot = $state(shots[0]);

	const security = [
		['Master password never stored', 'Vault keys are derived on unlock, memory-locked (mlock / VirtualLock) and zeroized on drop.'],
		['Locks itself', 'Vaults auto-lock on idle and defensively on OS suspend. Revealing a password requires re-entering the master password.'],
		['Host keys verified first', 'OpenSSH-compatible known_hosts with trust-on-first-use, changed-key detection and revocation — credentials are never sent before the host checks out.'],
		['Pastejacking protection', 'Multi-line or control-character pastes pop a confirmation showing exactly what would run.'],
		['Signed auto-updates', 'Every update is verified against a minisign public key embedded in the app. macOS builds are Developer ID signed and notarized.'],
		['No account. No telemetry.', 'No sign-up, no analytics SDK, no phone-home. Your profiles are local files you can export — encrypted.']
	];

	const downloads = [
		{ os: 'macOS', code: 'darwin', detail: 'Universal — Apple Silicon + Intel', art: '.dmg, signed & notarized' },
		{ os: 'Windows', code: 'win32', detail: 'x64', art: '.exe · .msi' },
		{ os: 'Linux', code: 'linux', detail: 'x64', art: '.deb · .rpm · .AppImage' }
	];
</script>

<!-- ── Nav ──────────────────────────────────────────────────────────────── -->
<header class="nav wrap">
	<a class="brand" href="/">
		<img src="/icon.svg" alt="" width="30" height="30" />
		<span>vaulterm<em>.dev</em></span>
	</a>
	<nav>
		<a href="#features">Features</a>
		<a href="#tour">Tour</a>
		<a href="#security">Security</a>
		<a href="#download">Download</a>
		<a class="gh" href={REPO} target="_blank" rel="noreferrer">GitHub ↗</a>
	</nav>
</header>

<!-- ── Hero ─────────────────────────────────────────────────────────────── -->
<section class="hero wrap">
	<div class="hero-copy">
		<p class="kicker hero-stagger" style="--i: 0">Native · macOS / Windows / Linux · open source</p>
		<h1 class="hero-stagger" style="--i: 1">
			The SSH workspace with a <span class="flame-text">vault at its core</span>.
		</h1>
		<p class="lede hero-stagger" style="--i: 2">
			Terminal, dual-pane SFTP, tunnels, Docker and Proxmox panels, remote monitoring —
			and your credentials sealed in an encrypted vault that locks itself.
			One native app, no account, no telemetry.
		</p>
		<div class="cta hero-stagger" style="--i: 3">
			<a class="btn btn-primary" href={RELEASES} target="_blank" rel="noreferrer">▼ Download free</a>
			<a class="btn btn-ghost" href="#tour">See it running</a>
		</div>
	</div>
	<div class="hero-shot hero-stagger" style="--i: 2">
		<img src="/shots/terminal.png" alt="VaulTerm: a real terminal session — Docker container list over SSH, profile sidebar with production and staging groups" loading="eager" />
	</div>
</section>

<!-- ── Pillars ──────────────────────────────────────────────────────────── -->
<section id="features" class="features wrap">
	<p class="kicker" use:reveal>What ships in the box</p>
	<h2 use:reveal>Six tools that usually live in six apps.</h2>
	<div class="pillar-grid">
		{#each pillars as f, i}
			<article class="pillar reveal" use:reveal style="transition-delay: {(i % 3) * 90}ms">
				<span class="pillar-tag">❯ {f.tag}</span>
				<h3>{f.heading}</h3>
				<p>{f.body}</p>
				<ul>
					{#each f.points as p}
						<li><span class="tick">·</span>{p}</li>
					{/each}
				</ul>
			</article>
		{/each}
	</div>
</section>

<!-- ── Screenshot tour ──────────────────────────────────────────────────── -->
<section id="tour" class="tour">
	<div class="wrap">
		<p class="kicker" use:reveal>Straight from the app</p>
		<h2 use:reveal>Real screenshots. Lamplight theme.<br />Five more themes inside.</h2>
		<div class="tour-tabs reveal" use:reveal role="tablist" aria-label="App screenshots">
			{#each shots as s}
				<button
					role="tab"
					aria-selected={activeShot.id === s.id}
					class:active={activeShot.id === s.id}
					onclick={() => (activeShot = s)}
				>
					{s.label}
				</button>
			{/each}
		</div>
		<div class="tour-frame reveal" use:reveal>
			<img src={activeShot.src} alt={activeShot.alt} loading="lazy" />
		</div>
		<p class="tour-note reveal" use:reveal>
			English & Spanish UI · themes: Lamplight, Dark, Solarized, Gruvbox, Catppuccin, Nord
		</p>
	</div>
</section>

<!-- ── Security ─────────────────────────────────────────────────────────── -->
<section id="security" class="security">
	<div class="wrap security-grid">
		<div class="security-lead reveal" use:reveal>
			<p class="kicker">Threat model included</p>
			<h2>Built like it will be attacked.</h2>
			<p class="lede">
				A terminal that stores credentials has one job above all others: don't leak them.
				These aren't roadmap promises — every item is in the code today.
			</p>
		</div>
		<ul class="security-list">
			{#each security as [t, d], i}
				<li class="reveal" use:reveal style="transition-delay: {i * 60}ms">
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
	<p class="kicker" use:reveal>Get VaulTerm</p>
	<h2 use:reveal>Pick your platform. Updates handle themselves.</h2>
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
				<span class="dl-code"><i>❯</i> {d.code}</span>
				<strong>{d.os}</strong>
				<span class="dl-detail">{d.detail}</span>
				<span class="dl-art">{d.art}</span>
				<span class="dl-go">download ❯</span>
			</a>
		{/each}
	</div>
	<p class="rename-note reveal" use:reveal>
		Heads-up: VaulTerm is the new name of NexTerm — current downloads still ship under the old
		name until the v0.5.0 rename release.
	</p>
</section>

<!-- ── Footer ───────────────────────────────────────────────────────────── -->
<footer class="footer">
	<div class="wrap footer-grid">
		<a class="brand" href="/">
			<img src="/icon.svg" alt="" width="26" height="26" />
			<span>vaulterm<em>.dev</em></span>
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

	.brand img { border-radius: 7px; }

	.brand em {
		font-style: normal;
		color: var(--amber);
	}

	.nav nav {
		display: flex;
		gap: 1.7rem;
		font-family: var(--mono);
		font-size: 0.84rem;
	}

	.nav nav a {
		text-decoration: none;
		color: var(--ink-dim);
		transition: color 0.2s;
	}

	.nav nav a:hover { color: var(--amber-soft); }
	.nav nav a.gh { color: var(--ink); }

	/* ── Hero ── */
	.hero {
		display: grid;
		grid-template-columns: 0.85fr 1.15fr;
		gap: 3.2rem;
		align-items: center;
		padding-block: 4.5rem 5.5rem;
	}

	.hero h1 {
		font-size: clamp(2.3rem, 4.6vw, 3.7rem);
		line-height: 1.06;
		margin-block: 1.1rem 1.3rem;
	}

	.lede {
		color: var(--ink-dim);
		font-size: 1.06rem;
		max-width: 34rem;
	}

	.cta {
		display: flex;
		gap: 0.9rem;
		margin-top: 2rem;
		flex-wrap: wrap;
	}

	.hero-shot img {
		width: 100%;
		display: block;
		border-radius: var(--radius);
		border: 1px solid var(--panel-edge);
		box-shadow:
			0 40px 90px rgba(0, 0, 0, 0.65),
			0 0 70px rgba(233, 163, 88, 0.09);
	}

	.hero-stagger {
		animation: rise 0.85s var(--ease-out) both;
		animation-delay: calc(var(--i) * 110ms);
	}

	@keyframes rise {
		from { opacity: 0; transform: translateY(24px); }
	}

	/* ── Sections shared ── */
	section h2 {
		font-size: clamp(1.8rem, 3.4vw, 2.7rem);
		line-height: 1.12;
		margin-block: 0.7rem 2.4rem;
		max-width: 46rem;
	}

	/* ── Pillars ── */
	.features { padding-block: 5.5rem 4.5rem; }

	.pillar-grid {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 1.3rem;
	}

	.pillar {
		background: linear-gradient(165deg, rgba(27, 23, 20, 0.9), rgba(16, 14, 11, 0.92));
		border: 1px solid var(--panel-edge);
		border-radius: var(--radius);
		padding: 1.5rem 1.5rem 1.6rem;
		transition: transform 0.35s var(--ease-out), border-color 0.35s, box-shadow 0.35s;
	}

	.pillar:hover {
		transform: translateY(-6px);
		border-color: rgba(233, 163, 88, 0.42);
		box-shadow: 0 24px 60px rgba(0, 0, 0, 0.6), 0 0 50px rgba(233, 163, 88, 0.08);
	}

	.pillar-tag {
		font-family: var(--mono);
		font-size: 0.72rem;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: var(--amber);
	}

	.pillar h3 {
		font-size: 1.32rem;
		margin-block: 0.7rem 0.6rem;
	}

	.pillar > p {
		color: var(--ink-dim);
		font-size: 0.94rem;
		margin-bottom: 1.1rem;
	}

	.pillar ul {
		list-style: none;
		display: grid;
		gap: 0.5rem;
	}

	.pillar li {
		display: flex;
		gap: 0.6rem;
		font-family: var(--mono);
		font-size: 0.79rem;
		color: var(--ink-dim);
	}

	.tick { color: var(--amber); font-weight: 700; }

	/* ── Tour ── */
	.tour {
		padding-block: 5.5rem;
		background:
			radial-gradient(ellipse 55rem 30rem at 80% 20%, rgba(233, 163, 88, 0.06), transparent 60%),
			linear-gradient(180deg, transparent, rgba(22, 19, 16, 0.8) 15%, rgba(22, 19, 16, 0.8) 85%, transparent);
	}

	.tour-tabs {
		display: flex;
		gap: 0.5rem;
		margin-bottom: 1.4rem;
		flex-wrap: wrap;
	}

	.tour-tabs button {
		font-family: var(--mono);
		font-size: 0.82rem;
		color: var(--ink-dim);
		background: rgba(27, 23, 20, 0.7);
		border: 1px solid var(--panel-edge);
		border-radius: 9px;
		padding: 0.5rem 1.1rem;
		cursor: pointer;
		transition: color 0.2s, border-color 0.2s, background 0.2s;
	}

	.tour-tabs button:hover { color: var(--ink); }

	.tour-tabs button.active {
		color: #1a1208;
		background: var(--flame);
		border-color: transparent;
		font-weight: 700;
	}

	.tour-frame img {
		width: 100%;
		display: block;
		border-radius: var(--radius);
		border: 1px solid var(--panel-edge);
		box-shadow: 0 40px 90px rgba(0, 0, 0, 0.65);
	}

	.tour-note {
		margin-top: 1.1rem;
		font-family: var(--mono);
		font-size: 0.78rem;
		color: var(--ink-faint);
		letter-spacing: 0.04em;
	}

	/* ── Security ── */
	.security { padding-block: 6rem 5rem; }

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
	}

	.security-list li {
		display: flex;
		gap: 1.4rem;
		padding: 1.25rem 0.4rem;
		border-bottom: 1px dashed rgba(233, 163, 88, 0.2);
	}

	.security-list li:first-child { border-top: 1px dashed rgba(233, 163, 88, 0.2); }

	.sec-index {
		font-family: var(--mono);
		font-size: 0.78rem;
		color: var(--amber);
		padding-top: 0.25rem;
	}

	.security-list strong {
		display: block;
		font-family: var(--body);
		font-size: 1.02rem;
		margin-bottom: 0.2rem;
	}

	.security-list p {
		color: var(--ink-dim);
		font-size: 0.92rem;
	}

	/* ── Download ── */
	.download { padding-block: 5.5rem 4.5rem; }

	.dl-grid {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 1.3rem;
	}

	.dl-card {
		display: grid;
		gap: 0.3rem;
		padding: 1.7rem 1.6rem;
		text-decoration: none;
		background: linear-gradient(165deg, rgba(27, 23, 20, 0.9), rgba(16, 14, 11, 0.92));
		border: 1px solid var(--panel-edge);
		border-radius: var(--radius);
		transition: transform 0.35s var(--ease-out), border-color 0.35s, box-shadow 0.35s;
	}

	.dl-card:hover {
		transform: translateY(-6px);
		border-color: rgba(240, 181, 111, 0.5);
		box-shadow: 0 24px 60px rgba(0, 0, 0, 0.6), 0 0 50px rgba(233, 163, 88, 0.1);
	}

	.dl-code {
		font-family: var(--mono);
		font-size: 0.78rem;
		color: var(--ink-faint);
		letter-spacing: 0.1em;
		margin-bottom: 0.5rem;
	}

	.dl-code i {
		font-style: normal;
		color: var(--amber);
		font-weight: 700;
	}

	.dl-card strong {
		font-family: var(--display);
		font-size: 1.45rem;
		font-weight: 600;
	}

	.dl-detail { color: var(--ink-dim); font-size: 0.92rem; }

	.dl-art {
		font-family: var(--mono);
		font-size: 0.76rem;
		color: var(--ink-faint);
		margin-top: 0.4rem;
	}

	.dl-go {
		font-family: var(--mono);
		font-size: 0.82rem;
		color: var(--amber);
		margin-top: 1rem;
		transition: letter-spacing 0.3s var(--ease-out);
	}

	.dl-card:hover .dl-go { letter-spacing: 0.08em; }

	.rename-note {
		margin-top: 1.8rem;
		font-size: 0.88rem;
		color: var(--ink-faint);
		max-width: 38rem;
	}

	/* ── Footer ── */
	.footer {
		border-top: 1px solid rgba(233, 163, 88, 0.15);
		padding-block: 2.3rem;
		background: rgba(13, 11, 9, 0.85);
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

	.foot-mono { font-family: var(--mono); color: var(--ink-faint); }

	.foot-cursor {
		display: inline-block;
		width: 0.55em;
		height: 1.05em;
		margin-left: 4px;
		background: var(--amber);
		vertical-align: text-bottom;
		animation: blink 1.05s steps(1) infinite;
	}

	@keyframes blink { 50% { opacity: 0; } }

	/* ── Responsive ── */
	@media (max-width: 940px) {
		.hero {
			grid-template-columns: 1fr;
			padding-block: 3rem 3.5rem;
			gap: 2.5rem;
		}

		.pillar-grid, .dl-grid { grid-template-columns: 1fr; }

		.security-grid {
			grid-template-columns: 1fr;
			gap: 2.5rem;
		}

		.security-lead { position: static; }

		.nav nav { gap: 1rem; font-size: 0.76rem; flex-wrap: wrap; }
	}
</style>
