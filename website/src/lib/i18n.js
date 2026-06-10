export const translations = {
	en: {
		meta: {
			title: 'VaulTerm — the SSH workspace with a vault at its core',
			description:
				'VaulTerm is a native SSH client for macOS, Windows and Linux: terminal with tabs and split panes, dual-pane SFTP, SSH tunnels, encrypted credential vault and password manager, Docker and Proxmox panels, remote monitoring. No account, no telemetry.'
		},
		nav: { features: 'Features', tour: 'Tour', security: 'Security', download: 'Download' },
		hero: {
			kicker: 'Native · macOS / Windows / Linux · open source',
			titlePre: 'The SSH workspace with a ',
			titleAccent: 'vault at its core',
			titlePost: '.',
			lede: 'Terminal, dual-pane SFTP, tunnels, Docker and Proxmox panels, remote monitoring — and your credentials sealed in an encrypted vault that locks itself. One native app, no account, no telemetry.',
			ctaDownload: '▼ Download free',
			ctaTour: 'See it running',
			shotAlt:
				'VaulTerm: a real terminal session — Docker container list over SSH, profile sidebar with production and staging groups'
		},
		features: {
			kicker: 'What ships in the box',
			title: 'Six tools that usually live in six apps.',
			pillars: [
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
			]
		},
		tour: {
			kicker: 'Straight from the app',
			title: 'Real screenshots. Lamplight theme. Five more themes inside.',
			note: 'English & Spanish UI · themes: Lamplight, Dark, Solarized, Gruvbox, Catppuccin, Nord',
			tabs: { terminal: 'Terminal', sftp: 'SFTP', launchpad: 'Launchpad', profile: 'Profiles' },
			alts: {
				terminal: 'VaulTerm terminal session with Docker output and the profile sidebar',
				sftp: 'Dual-pane SFTP browser with local and remote files',
				launchpad: 'Launchpad with saved profiles and recent connections',
				profile: 'Connection profile editor'
			}
		},
		security: {
			kicker: 'Threat model included',
			title: 'Built like it will be attacked.',
			lede: "A terminal that stores credentials has one job above all others: don't leak them. These aren't roadmap promises — every item is in the code today.",
			items: [
				['Master password never stored', 'Vault keys are derived on unlock, memory-locked (mlock / VirtualLock) and zeroized on drop.'],
				['Locks itself', 'Vaults auto-lock on idle and defensively on OS suspend. Revealing a password requires re-entering the master password.'],
				['Host keys verified first', 'OpenSSH-compatible known_hosts with trust-on-first-use, changed-key detection and revocation — credentials are never sent before the host checks out.'],
				['Pastejacking protection', 'Multi-line or control-character pastes pop a confirmation showing exactly what would run.'],
				['Signed auto-updates', 'Every update is verified against a minisign public key embedded in the app. macOS builds are Developer ID signed and notarized.'],
				['No account. No telemetry.', 'No sign-up, no analytics SDK, no phone-home. Your profiles are local files you can export — encrypted.']
			]
		},
		download: {
			kicker: 'Get VaulTerm',
			title: 'Pick your platform. Updates handle themselves.',
			go: 'download ❯',
			platforms: [
				{ os: 'macOS', code: 'darwin', detail: 'Universal — Apple Silicon + Intel', art: '.dmg, signed & notarized' },
				{ os: 'Windows', code: 'win32', detail: 'x64', art: '.exe · .msi' },
				{ os: 'Linux', code: 'linux', detail: 'x64', art: '.deb · .rpm · .AppImage' }
			],
			renameNote:
				'Heads-up: VaulTerm is the new name of NexTerm — current downloads still ship under the old name until the v0.5.0 rename release.'
		},
		footer: { line: 'Built with Rust + Tauri. Open source on' }
	},

	es: {
		meta: {
			title: 'VaulTerm — el espacio de trabajo SSH con una bóveda en el centro',
			description:
				'VaulTerm es un cliente SSH nativo para macOS, Windows y Linux: terminal con pestañas y paneles divididos, SFTP de doble panel, túneles SSH, bóveda de credenciales cifrada y gestor de contraseñas, paneles de Docker y Proxmox, monitoreo remoto. Sin cuentas, sin telemetría.'
		},
		nav: { features: 'Funciones', tour: 'Tour', security: 'Seguridad', download: 'Descargar' },
		hero: {
			kicker: 'Nativa · macOS / Windows / Linux · código abierto',
			titlePre: 'El espacio de trabajo SSH con una ',
			titleAccent: 'bóveda en el centro',
			titlePost: '.',
			lede: 'Terminal, SFTP de doble panel, túneles, paneles de Docker y Proxmox, monitoreo remoto — y tus credenciales selladas en una bóveda cifrada que se bloquea sola. Una sola aplicación nativa, sin cuentas, sin telemetría.',
			ctaDownload: '▼ Descargar gratis',
			ctaTour: 'Verla en acción',
			shotAlt:
				'VaulTerm: una sesión de terminal real — lista de contenedores Docker por SSH, barra lateral de perfiles con grupos de producción y staging'
		},
		features: {
			kicker: 'Todo lo que incluye',
			title: 'Seis herramientas que normalmente viven en seis aplicaciones.',
			pillars: [
				{
					tag: 'terminal',
					heading: 'Una terminal que sigue el ritmo',
					body: 'xterm.js sobre WebGL, pestañas y paneles divididos por sesión, búsqueda en la terminal, snippets de comandos con variables y difusión de teclado a todos los paneles cuando lo necesitas.',
					points: ['Pestañas + paneles redimensionables', 'Snippets con {{variables}}', 'Difusión de entrada entre paneles']
				},
				{
					tag: 'sftp',
					heading: 'Archivos sin salir',
					body: 'Navegador local/remoto de doble panel con arrastrar y soltar en ambas direcciones, descargas recursivas de carpetas con progreso, resolución de conflictos y edición de archivos remotos en la app.',
					points: ['Transferencias drag & drop', 'Descargas recursivas', 'Ver y editar archivos remotos']
				},
				{
					tag: 'bóveda',
					heading: 'Dos bóvedas, cero texto plano',
					body: 'Las credenciales SSH y tus contraseñas personales viven en almacenes separados, cada uno cifrado con AES-256-GCM tras una contraseña maestra con Argon2id. Bloqueo automático por inactividad, re-autenticación para revelar.',
					points: ['AES-256-GCM + Argon2id', 'Auto-bloqueo y bloqueo al suspender', 'Importar / exportar Bitwarden']
				},
				{
					tag: 'túneles',
					heading: 'Túneles que puedes ver',
					body: 'Reenvío de puertos local y remoto más un proxy dinámico SOCKS5 completo, guardados por perfil, cada uno con estado en vivo, conexiones activas y bytes de entrada/salida.',
					points: ['-L, -R y SOCKS5 (-D)', 'Guardados con cada perfil', 'Contadores de tráfico en vivo']
				},
				{
					tag: 'paneles',
					heading: 'Tu flota en una pantalla',
					body: 'Monitoreo remoto con gráficos de CPU, memoria, disco y red. Contenedores Docker: iniciar, detener, logs. Proxmox LXC y QEMU: ciclo de vida y snapshots.',
					points: ['Gráficos de monitoreo', 'Docker por SSH', 'Snapshots de Proxmox LXC + QEMU']
				},
				{
					tag: 'identidad',
					heading: 'Llaves, bien hechas',
					body: 'Autenticación por contraseña, llave, agente y keyboard-interactive. Genera llaves Ed25519, RSA o ECDSA en la app. Soporte de bastión ProxyJump y verificación known_hosts de OpenSSH.',
					points: ['Soporte ssh-agent y MFA', 'Generador de llaves (Ed25519…)', 'Confianza de host en el primer uso']
				}
			]
		},
		tour: {
			kicker: 'Directo de la aplicación',
			title: 'Capturas reales. Tema Lamplight. Cinco temas más incluidos.',
			note: 'Interfaz en español e inglés · temas: Lamplight, Dark, Solarized, Gruvbox, Catppuccin, Nord',
			tabs: { terminal: 'Terminal', sftp: 'SFTP', launchpad: 'Launchpad', profile: 'Perfiles' },
			alts: {
				terminal: 'Sesión de terminal de VaulTerm con salida de Docker y la barra lateral de perfiles',
				sftp: 'Navegador SFTP de doble panel con archivos locales y remotos',
				launchpad: 'Launchpad con perfiles guardados y conexiones recientes',
				profile: 'Editor de perfiles de conexión'
			}
		},
		security: {
			kicker: 'Con modelo de amenazas incluido',
			title: 'Construida como si fuera a ser atacada.',
			lede: 'Una terminal que guarda credenciales tiene un trabajo por encima de todos: no filtrarlas. No son promesas de roadmap — cada punto está en el código hoy.',
			items: [
				['La contraseña maestra nunca se guarda', 'Las llaves de la bóveda se derivan al desbloquear, se fijan en memoria (mlock / VirtualLock) y se borran a cero al liberarse.'],
				['Se bloquea sola', 'Las bóvedas se bloquean por inactividad y, de forma defensiva, al suspender el sistema. Revelar una contraseña exige reingresar la contraseña maestra.'],
				['Llaves de host verificadas primero', 'known_hosts compatible con OpenSSH: confianza en el primer uso, detección de llaves cambiadas y revocación — las credenciales nunca se envían antes de validar el host.'],
				['Protección contra pastejacking', 'Pegar texto multilínea o con caracteres de control abre una confirmación que muestra exactamente qué se ejecutaría.'],
				['Actualizaciones firmadas', 'Cada actualización se verifica contra una llave pública minisign embebida en la app. Los builds de macOS van firmados con Developer ID y notarizados.'],
				['Sin cuentas. Sin telemetría.', 'Sin registro, sin SDK de analytics, sin llamadas a casa. Tus perfiles son archivos locales que puedes exportar — cifrados.']
			]
		},
		download: {
			kicker: 'Descarga VaulTerm',
			title: 'Elige tu plataforma. Las actualizaciones se manejan solas.',
			go: 'descargar ❯',
			platforms: [
				{ os: 'macOS', code: 'darwin', detail: 'Universal — Apple Silicon + Intel', art: '.dmg, firmado y notarizado' },
				{ os: 'Windows', code: 'win32', detail: 'x64', art: '.exe · .msi' },
				{ os: 'Linux', code: 'linux', detail: 'x64', art: '.deb · .rpm · .AppImage' }
			],
			renameNote:
				'Aviso: VaulTerm es el nuevo nombre de NexTerm — las descargas actuales todavía usan el nombre anterior hasta el release v0.5.0.'
		},
		footer: { line: 'Hecho con Rust + Tauri. Código abierto en' }
	}
};
