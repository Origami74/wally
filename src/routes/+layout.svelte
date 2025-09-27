<script>
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	
	let currentPath = '';
	
	$: currentPath = $page.url.pathname;
	
	function navigateTo(path) {
		goto(path);
	}
</script>

<main class="app">
	<div class="content">
		<slot />
	</div>
	
	<nav class="bottom-nav">
		<button 
			class="nav-item" 
			class:active={currentPath === '/'}
			on:click={() => navigateTo('/')}
		>
			<div class="nav-icon">🏠</div>
			<span>Home</span>
		</button>
		
		<button 
			class="nav-item" 
			class:active={currentPath === '/developer'}
			on:click={() => navigateTo('/developer')}
		>
			<div class="nav-icon">🔧</div>
			<span>Developer</span>
		</button>
	</nav>
</main>

<style>
	:global(html, body) {
		margin: 0;
		padding: 0;
		height: 100%;
		font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
		background: #2D1B69; /* Nostr purple base */
		color: white;
		overflow: hidden;
	}
	
	:global(#app) {
		height: 100vh;
		display: flex;
		flex-direction: column;
	}

	.app {
		display: flex;
		flex-direction: column;
		height: 100vh;
		background: linear-gradient(135deg, #2D1B69 0%, #1a0f3a 100%); /* Nostr purple gradient */
		color: white;
		font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
		position: relative;
		overflow: hidden;
	}
	
	/* Add the MeshMate background pattern */
	.app::before {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		background-image: url('/meshmate-bg-purple.webp');
		background-size: cover;
		background-position: center;
		background-repeat: no-repeat;
		opacity: 0.3;
		pointer-events: none;
		z-index: 0;
	}
	
	
	.content {
		flex: 1;
		overflow-y: auto;
		padding: 0;
		padding-bottom: 80px; /* Space for bottom nav */
		position: relative;
		z-index: 1;
	}
	
	.bottom-nav {
		position: fixed;
		bottom: 0;
		left: 0;
		right: 0;
		height: 70px;
		background: rgba(255, 140, 0, 0.222); /* Much more subtle orange */
		backdrop-filter: blur(10px);
		display: flex;
		justify-content: space-around;
		align-items: center;
		border-top: 1px solid rgba(255, 140, 0, 0.2);
		z-index: 2;
	}
	
	.nav-item {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 4px;
		background: none;
		border: none;
		color: rgba(255, 255, 255, 0.8);
		cursor: pointer;
		padding: 8px 16px;
		border-radius: 12px;
		transition: all 0.3s ease;
		min-width: 60px;
	}
	
	.nav-item:hover {
		color: white;
		background: rgba(255, 255, 255, 0.1);
		transform: translateY(-1px);
	}
	
	.nav-item.active {
		color: white;
		background: rgba(255, 255, 255, 0.2);
		transform: translateY(-2px);
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
	}
	
	.nav-icon {
		font-size: 22px;
		filter: drop-shadow(0 1px 2px rgba(0, 0, 0, 0.3));
	}
	
	.nav-item span {
		font-size: 12px;
		font-weight: 600;
		text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
	}
</style>