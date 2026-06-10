<script>
	import { onMount } from 'svelte';

	/**
	 * Self-typing terminal. Cycles through scenes that demo the three pillars:
	 * SSH sessions, the encrypted vault, and Proxmox control.
	 * Each step is either a typed command or printed output lines.
	 */
	const scenes = [
		[
			{ cmd: 'nexterm connect prod-web-01' },
			{
				out: [
					{ text: '⠿ ed25519 key accepted · session opened in 84ms', cls: 'ok' },
					{ text: 'prod-web-01 · Ubuntu 24.04 LTS · 3 sessions pinned', cls: 'dim' }
				]
			},
			{ cmd: 'uptime' },
			{ out: [{ text: '23:41 up 312 days · load 0.42 0.38 0.35', cls: '' }] }
		],
		[
			{ cmd: 'vault unlock' },
			{
				out: [
					{ text: 'master password ·········· ✓ argon2id', cls: 'ok' },
					{ text: '12 entries · AES-256-GCM · auto-lock in 60s', cls: 'dim' }
				]
			},
			{ cmd: 'vault copy prod-db/root' },
			{ out: [{ text: '✓ copied — clipboard clears in 30s', cls: 'ok' }] }
		],
		[
			{ cmd: 'proxmox ls' },
			{
				out: [
					{ text: ' 100  web-frontend   qemu  running   4G', cls: '' },
					{ text: ' 101  postgres-16    qemu  running   8G', cls: '' },
					{ text: ' 207  ci-runner      lxc   stopped   2G', cls: 'dim' }
				]
			},
			{ cmd: 'proxmox snapshot 101 pre-upgrade' },
			{ out: [{ text: '✓ snapshot created on pve-01', cls: 'ok' }] }
		]
	];

	let lines = $state([]);
	let typing = $state('');

	const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

	onMount(() => {
		let alive = true;

		(async () => {
			let scene = 0;
			while (alive) {
				lines = [];
				for (const step of scenes[scene]) {
					if (!alive) return;
					if (step.cmd) {
						typing = '';
						await sleep(420);
						for (const ch of step.cmd) {
							if (!alive) return;
							typing += ch;
							await sleep(28 + Math.sin(typing.length) * 14);
						}
						await sleep(260);
						lines = [...lines, { text: typing, cls: 'cmd' }];
						typing = '';
					} else if (step.out) {
						for (const l of step.out) {
							if (!alive) return;
							await sleep(150);
							lines = [...lines, l];
						}
					}
				}
				await sleep(3200);
				scene = (scene + 1) % scenes.length;
			}
		})();

		return () => {
			alive = false;
		};
	});
</script>

<div class="term" role="img" aria-label="Animated demo of NexTerm: SSH session, vault unlock and Proxmox snapshot commands">
	<div class="term-bar">
		<span class="dot red"></span>
		<span class="dot yellow"></span>
		<span class="dot green"></span>
		<span class="term-title">nexterm — prod-web-01</span>
		<span class="term-lock" title="encrypted session">⌁</span>
	</div>
	<div class="term-body">
		{#each lines as line}
			<div class="line {line.cls}">
				{#if line.cls === 'cmd'}<span class="prompt">❯</span>{/if}
				<span>{line.text}</span>
			</div>
		{/each}
		<div class="line cmd live">
			<span class="prompt">❯</span>
			<span>{typing}</span><span class="cursor"></span>
		</div>
	</div>
</div>

<style>
	.term {
		font-family: var(--mono);
		font-size: 0.86rem;
		background: linear-gradient(170deg, rgba(16, 23, 41, 0.92), rgba(7, 11, 21, 0.96));
		border: 1px solid var(--panel-edge);
		border-radius: var(--radius);
		box-shadow:
			0 40px 90px rgba(2, 6, 16, 0.8),
			0 0 70px rgba(56, 189, 246, 0.13),
			inset 0 1px 0 rgba(255, 255, 255, 0.05);
		backdrop-filter: blur(14px);
		overflow: hidden;
	}

	.term-bar {
		display: flex;
		align-items: center;
		gap: 7px;
		padding: 0.7rem 1rem;
		border-bottom: 1px solid rgba(99, 130, 199, 0.12);
		background: rgba(10, 15, 30, 0.65);
	}

	.dot {
		width: 11px;
		height: 11px;
		border-radius: 50%;
		opacity: 0.85;
	}
	.dot.red { background: #ff5f57; }
	.dot.yellow { background: #febc2e; }
	.dot.green { background: #28c840; }

	.term-title {
		margin-left: 0.8rem;
		font-size: 0.74rem;
		color: var(--ink-faint);
		letter-spacing: 0.04em;
	}

	.term-lock {
		margin-left: auto;
		color: var(--cyan);
		opacity: 0.7;
	}

	.term-body {
		padding: 1.1rem 1.2rem 1.3rem;
		min-height: 248px;
		display: flex;
		flex-direction: column;
		gap: 0.42rem;
	}

	.line {
		display: flex;
		gap: 0.55rem;
		color: var(--ink-dim);
		animation: line-in 0.25s var(--ease-out);
		white-space: pre-wrap;
	}

	.line.cmd { color: var(--ink); }
	.line.ok { color: #7ee2b8; }
	.line.dim { color: var(--ink-faint); }

	.prompt {
		color: var(--blue);
		font-weight: 700;
	}

	.cursor {
		display: inline-block;
		width: 0.58em;
		height: 1.15em;
		margin-left: 2px;
		background: var(--cyan);
		vertical-align: text-bottom;
		animation: blink 1.05s steps(1) infinite;
		box-shadow: 0 0 10px rgba(103, 232, 249, 0.8);
	}

	@keyframes blink {
		50% { opacity: 0; }
	}

	@keyframes line-in {
		from { opacity: 0; transform: translateY(4px); }
	}
</style>
