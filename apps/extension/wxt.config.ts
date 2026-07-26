import { defineConfig } from 'wxt';
import { SUPPORTED_HOSTS } from './src/lib/sites';

export default defineConfig({
  srcDir: 'src',
  modules: ['@wxt-dev/module-svelte'],
  manifest: {
    name: 'Sonata',
    description: 'Publica o que você está ouvindo/assistindo no navegador para o daemon Sonata.',
    permissions: ['storage', 'nativeMessaging', 'tabs'],
    icons: {
      16: 'icon/16.png',
      32: 'icon/32.png',
      48: 'icon/48.png',
      128: 'icon/128.png',
    },
    host_permissions: SUPPORTED_HOSTS.map((host) => `https://${host}/*`),
  },
});