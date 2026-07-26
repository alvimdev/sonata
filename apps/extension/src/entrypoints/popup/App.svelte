<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import type { DaemonStatus } from '@/lib/status';
  import { STATUS_REQUEST_TYPE } from '@/lib/status';
  import { SUPPORTED_HOSTS } from '@/lib/sites';

  let status: DaemonStatus = { connected: false, track: null };
  let interval: ReturnType<typeof setInterval>;

  async function poll() {
    try {
      const response = await browser.runtime.sendMessage({ type: STATUS_REQUEST_TYPE });
      if (response) status = response;
    } catch {
      status = { connected: false, track: null };
    }
  }

  onMount(() => {
    poll();
    interval = setInterval(poll, 1000);
  });

  onDestroy(() => clearInterval(interval));

  $: statusLabel = !status.connected
    ? 'sem conexão com o daemon'
    : status.track
      ? status.track.state === 'playing'
        ? 'transmitindo'
        : 'pausado'
      : 'aguardando música';
</script>

<main>
  <header>
    <img src="/icon/48.png" alt="" class="logo" />
    <div>
      <h1>Sonata</h1>
      <div class="signal" class:live={status.connected}>
        <span class="bars" aria-hidden="true">
          <i></i><i></i><i></i><i></i><i></i>
        </span>
        <span class="signal-label">{statusLabel}</span>
      </div>
    </div>
  </header>

  {#if status.track}
    <section class="now-playing">
      <div class="artwork">
        {#if status.track.artworkUrl}
          <img src={status.track.artworkUrl} alt="" />
        {:else}
          <span class="artwork-fallback" aria-hidden="true">♪</span>
        {/if}
      </div>
      <div class="track-info">
        <p class="title">{status.track.title}</p>
        <p class="artist">{status.track.artists.join(', ') || 'Artista desconhecido'}</p>
      </div>
    </section>
  {:else}
    <section class="empty">
      <p>Nada tocando em nenhuma aba suportada agora.</p>
    </section>
  {/if}

  <footer>
    <p class="footer-label">sites suportados</p>
    <ul>
      {#each SUPPORTED_HOSTS as host}
        <li>{host}</li>
      {/each}
    </ul>
  </footer>
</main>

<style>
  :global(body) {
    margin: 0;
    width: 300px;
    background: #121317;
    color: #ededf2;
    font-family: system-ui, -apple-system, sans-serif;
  }

  main {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 16px;
  }

  header {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .logo {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    flex-shrink: 0;
  }

  h1 {
    font-size: 15px;
    font-weight: 600;
    margin: 0 0 3px;
    letter-spacing: -0.01em;
  }

  .signal {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .bars {
    display: inline-flex;
    align-items: flex-end;
    gap: 2px;
    height: 10px;
  }

  .bars i {
    display: block;
    width: 2px;
    background: #4a4b55;
    border-radius: 1px;
    height: 30%;
  }

  .signal.live .bars i {
    background: #4ade80;
    animation: pulse 1.1s ease-in-out infinite;
  }

  .bars i:nth-child(1) { height: 40%; animation-delay: 0ms; }
  .bars i:nth-child(2) { height: 70%; animation-delay: 120ms; }
  .bars i:nth-child(3) { height: 100%; animation-delay: 240ms; }
  .bars i:nth-child(4) { height: 60%; animation-delay: 360ms; }
  .bars i:nth-child(5) { height: 85%; animation-delay: 480ms; }

  @keyframes pulse {
    0%, 100% { transform: scaleY(0.6); opacity: 0.7; }
    50% { transform: scaleY(1); opacity: 1; }
  }

  @media (prefers-reduced-motion: reduce) {
    .bars i { animation: none !important; }
  }

  .signal-label {
    font-family: ui-monospace, 'SFMono-Regular', Menlo, monospace;
    font-size: 10.5px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: #8b8d98;
  }

  .signal.live .signal-label {
    color: #4ade80;
  }

  .now-playing {
    display: flex;
    gap: 10px;
    background: #1b1c22;
    border-radius: 10px;
    padding: 10px;
  }

  .artwork {
    width: 44px;
    height: 44px;
    border-radius: 6px;
    overflow: hidden;
    flex-shrink: 0;
    background: #26272f;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .artwork img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .artwork-fallback {
    color: #8c7cf0;
    font-size: 18px;
  }

  .track-info {
    min-width: 0;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 2px;
  }

  .title {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .artist {
    margin: 0;
    font-size: 12px;
    color: #8b8d98;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .empty {
    background: #1b1c22;
    border-radius: 10px;
    padding: 14px;
  }

  .empty p {
    margin: 0;
    font-size: 12px;
    color: #8b8d98;
    text-align: center;
  }

  footer {
    border-top: 1px solid #26272f;
    padding-top: 10px;
  }

  .footer-label {
    margin: 0 0 6px;
    font-family: ui-monospace, 'SFMono-Regular', Menlo, monospace;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: #5c5e69;
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  li {
    font-family: ui-monospace, 'SFMono-Regular', Menlo, monospace;
    font-size: 11px;
    color: #8b8d98;
  }
</style>