<!--
  Este arquivo é a versão em português do README. Quando a versão em inglês
  (README.en.md) for criada, ela deve espelhar este arquivo seção por seção,
  apenas traduzida — mesma estrutura, mesmo conteúdo, mesma ordem.
-->

<div align="center">
  <img src="./apps/extension/public/icon/128.png" alt="Ícone do Sonata" width="96" />

  # Sonata

  **Daemon multiplataforma que publica o que você está ouvindo através de sistemas de presença — começando pelo Discord Rich Presence.**

  [![Licença](https://img.shields.io/badge/licença-GPLv2-blue)](./LICENSE)
  ![Rust](https://img.shields.io/badge/Rust-workspace-orange)
  ![Plataformas](https://img.shields.io/badge/plataformas-Windows%20%7C%20Linux-informational)
  ![Status](https://img.shields.io/badge/status-em%20desenvolvimento-yellow)
</div>

<div align="center">

🇧🇷 **Português** · 🇺🇸 English *(em breve)*

</div>

---

## Sumário

- [Motivação](#motivação)
- [Stack](#stack)
- [Como funciona](#como-funciona)
- [Como rodar](#como-rodar)
- [Funcionalidades](#funcionalidades)
- [Créditos](#créditos)
- [Como contribuir](#como-contribuir)
- [Licença](#licença)

---

## Motivação

Sabe quando você está online no Discord, de bobeira, abre o Spotify e ele mostra pros seus amigos o que você está ouvindo — e como você é uma pessoa de bom gosto? Pois é. Quem usa YouTube Music não tem isso. E esse é o principal motivo do Sonata existir.

Depois da minha migração do Spotify pro YouTube Music, senti muita falta desse pequeno detalhe. Fui pesquisar bastante sobre RPCs pro YouTube Music por aí e, no fim, decidi fazer o meu próprio.

Incrível, não? Sim, é sim.

## Stack

| Camada | Tecnologia |
| --- | --- |
| Daemon | Rust (workspace com [Tokio](https://tokio.rs), assíncrono do início ao fim) |
| Captura no Windows | [`GlobalSystemMediaTransportControlsSessionManager`](https://learn.microsoft.com/uwp/api/windows.media.control) (WinRT/GSMTC) |
| Captura no Linux | MPRIS via [D-Bus](https://www.freedesktop.org/wiki/Specifications/mpris-spec/) (crate [`zbus`](https://docs.rs/zbus)) |
| Presença no Discord | IPC local via [`discord-rich-presence`](https://docs.rs/discord-rich-presence) |
| Extensão de navegador | TypeScript + [Svelte](https://svelte.dev) + [WXT](https://wxt.dev) (Manifest V3) |
| Ponte extensão ↔ daemon | [Native Messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging) (stdio local, sem rede) |

## Como funciona

O Sonata é dividido em duas peças conceituais, conectadas por um domínio comum:

```
MediaProvider  →  MediaEvent  →  PresencePublisher
```

- **Providers** capturam o que está tocando. Hoje existem dois:
  - **System** — lê a sessão de mídia ativa direto do sistema operacional (Windows/Linux). Funciona com qualquer app que exponha uma sessão de mídia nativa, mas não sabe diferenciar sites dentro de um navegador (ex: não distingue YouTube Music de Netflix, ambos aparecem como "Chrome").
  - **Browser** — uma extensão de navegador identifica exatamente o site (`music.youtube.com`, `soundcloud.com`, etc.) e envia os dados pro daemon via Native Messaging. Mais preciso, mas exige a extensão instalada.
- **Publishers** só recebem eventos e os traduzem pra um sistema de presença. Hoje existe um: **Discord Rich Presence**.

Só um provider fica ativo por vez — você escolhe qual usar no `sonata.toml`. Não há seleção automática nem fallback implícito.

## Como rodar

> Esta seção assume que você baixou os binários prontos na aba **[Releases](../../releases)** do repositório. Se você quer compilar o projeto você mesmo, veja [Como contribuir](#como-contribuir).

### 1. Baixe e configure

Baixe o executável do daemon mais recente na aba Releases e extraia numa pasta de sua preferência. Nessa mesma pasta deve existir um `sonata.toml` — exemplo mínimo:

```toml
provider = "system"   # ou "browser"

[discord]
client_id = "SEU_CLIENT_ID_AQUI"
```

### 2. Modo System — rodando em background

No modo `system`, o daemon fica escutando a sessão de mídia do seu sistema operacional o tempo todo — ele precisa continuar aberto enquanto você quiser que a presença seja atualizada.

**Ligar:** dá duplo-clique no executável, ou rode ele por um terminal se quiser ver os logs.

**Deixar sempre ligado ao iniciar o Windows (opcional):** cria um atalho do executável e coloca na pasta de Inicialização (`Win + R` → `shell:startup`).

**Desligar:** fecha a janela do terminal (se rodou por lá), ou encerra o processo pelo Gerenciador de Tarefas caso tenha rodado em background sem console visível.

### 3. Modo Browser — conectando a extensão

No modo `browser`, o daemon **não fica em background o tempo todo** — o navegador sobe o processo sozinho quando necessário e o encerra quando não há mais nada tocando. Passos:

1. Baixe o `.zip` da extensão na aba Releases e extraia numa pasta.
2. No seu navegador baseado em Chromium (Chrome, Edge, Vivaldi, Brave...), abra a página de extensões, ative o **Modo do desenvolvedor** e use **Carregar sem compactação**, apontando pra pasta extraída.
3. Copie o **ID da extensão** exibido no card dela.
4. Baixe o manifesto de Native Messaging (`id.zone.sonata.json`) da aba Releases, ajuste o campo `"path"` pro caminho do executável do daemon e o `"allowed_origins"` pro ID copiado no passo anterior.
5. Registre o manifesto no seu navegador:

   **Windows** (registro do sistema — repare que a maioria dos navegadores baseados em Chromium, incluindo o Vivaldi, lê a chave do **Chrome**, mesmo sem ser o Chrome):
   ```powershell
   $manifestPath = 'CAMINHO\PARA\id.zone.sonata.json'
   New-Item -Path "HKCU:\Software\Google\Chrome\NativeMessagingHosts\id.zone.sonata" -Force
   Set-ItemProperty -Path "HKCU:\Software\Google\Chrome\NativeMessagingHosts\id.zone.sonata" -Name "(Default)" -Value $manifestPath
   ```

   **Linux**, copie o manifesto para:
   ```
   ~/.config/google-chrome/NativeMessagingHosts/id.zone.sonata.json
   ```

6. Reinicie o navegador por completo (não só a aba/extensão).
7. Entre num site suportado (veja a lista na popup da extensão) e dê play — a presença deve aparecer no Discord em poucos segundos.

## Funcionalidades

Status honesto do que já existe, dividido por quanto foi de fato validado em uso real:

### ✅ Concluído e validado

- Provider **System** no Windows (GSMTC) — captura qualquer app com sessão de mídia nativa
- Publisher **Discord Rich Presence** — capa do álbum, badge do Sonata, timestamps de progresso, limpeza automática ao parar
- Provider **Browser** via Native Messaging (`connectNative`) — testado ponta a ponta
- Adapter de **music.youtube.com** na extensão
- Popup de status da extensão (conexão com o daemon, música atual, sites suportados)

### ⚠️ Implementado, mas pouco testado

- Provider **System** no Linux (MPRIS/D-Bus) — código escrito e com testes unitários, mas ainda não validado rodando de verdade
- Timeout de inatividade do provider Browser (limpa a presença após 30s sem eventos)
- Popup da extensão tem um bug visual conhecido: o card de "tocando agora" pode estourar a largura da popup quando o nome da música é muito grande

### 🚧 Planejado / não implementado

- Provider **System** no macOS (hoje retorna erro explícito de "não suportado")
- Adapters de **SoundCloud** e **Deezer** na extensão (só stubs por enquanto)
- Filtro de quais apps/sites geram presença (ex: ignorar Instagram/X, permitir só players de música)
- Botão clicável levando pra URL da faixa na Rich Presence (o campo `Track.url` já existe no domínio, só não é usado ainda)
- Outros publishers além do Discord (Steam, WebSocket, HTTP)
- Rotação do arquivo de log do daemon
- Versão em inglês deste README

## Créditos

| | Nome | GitHub |
| --- | --- | --- |
| <img src="https://github.com/alvimdev.png" width="48" height="48" alt="" style="border-radius:50%" /> | [Bernardo Alvim](https://github.com/alvimdev) | [@alvimdev](https://github.com/alvimdev) |

## Como contribuir

Contribuições são bem-vindas. Esta seção cobre como rodar o projeto **a partir do código-fonte**.

### Pré-requisitos

- [Rust](https://rustup.rs) (edição 2024/2026, via `rustup`)
- [Node.js](https://nodejs.org) + npm, pra extensão
- Windows: [`cargo-xwin`](https://github.com/rust-cross/cargo-xwin) se for cross-compilar a partir de Linux/WSL, ou o toolchain MSVC nativo
- Linux: `libdbus-1-dev` (ou equivalente da sua distro) pro provider MPRIS

### Daemon

```bash
git clone https://github.com/alvimdev/sonata.git
cd sonata
cargo build -p daemon --release
```

O binário sai em `target/release/` (ou `target/<seu-target>/release/` em builds cross-compiladas). Copie um `sonata.toml` pra pasta do executável antes de rodar — veja o exemplo em [Como rodar](#como-rodar).

### Extensão

```bash
cd apps/extension
npm install
npm run dev
```

Isso abre uma instância do navegador com a extensão já carregada e hot-reload ativo. Veja mais detalhes de arquitetura no cabeçalho de `apps/extension/src/entrypoints/`.

### Estrutura do workspace

```
sonata/
├── apps/
│   ├── daemon/              # bootstrap do daemon
│   └── extension/           # extensão de navegador (WXT + Svelte + TS)
└── crates/
    ├── sonata-core/             # domínio: modelos, traits, eventos
    ├── sonata-provider-system/  # captura via API nativa do SO
    ├── sonata-provider-browser/ # captura via Native Messaging
    └── sonata-rpc/               # publishers (Discord Rich Presence)
```

Antes de abrir uma PR, garanta que `cargo test` e `cargo clippy` passam no lado Rust, e `npm run check` no lado da extensão.

## Licença

Este projeto está sob a licença especificada em [`LICENSE`](./LICENSE).
