# 🔍 DeskLens — Relatório Histórico Completo de Desenvolvimento

> **DeskLens** é uma ferramenta de produtividade desktop ("Google Lens para Desktop") desenvolvida em **Tauri v2 (Rust + Vanilla HTML/CSS/JS)** compatível com **Windows 11/10** e **Linux (Wayland & X11)**.  
> Este documento registra detalhadamente todas as etapas, decisões arquiteturais, desafios técnicos enfrentados e soluções implementadas desde a criação do projeto até a versão atual.

---

## 📌 Sumário Executivo
- **Tecnologias**: Tauri v2, Rust 1.80+, Vanilla HTML5/CSS3/JavaScript (ESModules), `Windows.Media.Ocr` (Windows Runtime), Tesseract OCR (Linux), `xcap` (Screen Capture), `image` crate (Lanczos3 upscaling), Google Lens API / CDN Bridge.
- **Repositório GitHub**: [https://github.com/GustavoGeminiano/desklens.git](https://github.com/GustavoGeminiano/desklens.git)
- **Status de Verificação**: 97 testes automatizados aprovados (100% de sucesso) auditados com veredito formal `VICTORY CONFIRMED`.

---

## 🏗️ 1. Arquitetura e Estrutura do Projeto

```
desklens/
├── .github/
│   └── workflows/
│       └── release.yml                 # Pipeline CI/CD GitHub Actions multiplataforma
├── src-tauri/
│   ├── Cargo.toml                      # Dependências Rust (tauri, xcap, image, reqwest, arboard, windows)
│   ├── tauri.conf.json                 # Configurações de janelas (main, results), ícones e permissões
│   ├── icons/                          # Ícones do app (16x16, 32x32, 128x128, .ico, .icns)
│   ├── src/
│   │   ├── main.rs                     # Ponto de entrada nativo com supressão de console CMD
│   │   ├── lib.rs                      # Orquestrador do Tauri, System Tray, Global Hotkeys e IPC
│   │   ├── ocr.rs                      # Pipeline de OCR de alta precisão, pré-processamento e coalescência
│   │   └── translation.rs              # Motor de tradução na tela e normalização de idiomas
│   └── tests/
│       ├── code_ocr_tests.rs           # Testes de OCR de código Python em temas claros e escuros
│       ├── translation_tests.rs        # Testes de tradução e preservação de coordenadas
│       ├── linux_compat_tests.rs       # Testes de compatibilidade Wayland/X11 e monitores
│       ├── adversarial_tests.rs        # Testes de estresse, injeções e casos de borda
│       └── m1_backend_stress_tests.rs  # Testes de concorrência e carga do backend
├── src/
│   ├── index.html                      # Interface do Overlay de Congelamento e Hub de Configurações
│   ├── main.js                         # Controlador do overlay interativo, seleção e cópia de código
│   ├── styles.css                      # Estilos do HUD, Glow Frame, Corner Brackets e Dark Theme
│   ├── results.html                    # Painel lateral compacto de Busca Visual e Resultados OCR
│   ├── results.js                      # Controlador de integração com Google Lens e OCR lateral
│   └── results.css                     # Estilos do painel lateral com scrollbar moderna dark
├── tests/
│   ├── code_selection.test.mjs         # Testes automatizados de seleção, arrasto e cópia no DOM
│   ├── language_selection.test.mjs     # Testes de seleção e persistência de idiomas
│   ├── overlay.test.mjs                # Testes de renderização de caixas de tradução na tela
│   └── adversarial_overlay.test.mjs    # Testes adversariais contra ataques XSS e falhas de layout
├── run-linux.sh                        # Script de inicialização, trigger via socket e atalho no Linux
├── DESKLENS_PROJECT_HISTORY.md         # Este documento de histórico completo
└── README.md                           # Documentação pública e guia de uso
```

---

## 🚀 2. Linha do Tempo e Evolução por Sprints

### 🟢 Sprint 0: Fundação, Motor em Segundo Plano e Atalho Global
- **Objetivo**: Criar um motor leve e invisível que roda em segundo plano consumindo menos de 20 MB de RAM.
- **Implementações**:
  - Configuração do Tauri v2 com janela principal invisível e sem poluir a barra de tarefas.
  - Registro de ícone na **Bandeja do Sistema (*System Tray*)** com menu interativo:
    - *Capture Screen (Ctrl+Shift+X)*
    - *Settings*
    - *Quit*
  - Registro do atalho global nativo **`Ctrl + Shift + X`** via `tauri-plugin-global-shortcut`.
  - Tratamento de fechamento: ao clicar no `X` da janela, o app apenas se oculta na bandeja em vez de ser finalizado.

---

### 🟢 Sprint 1: Captura de Tela e Overlay Congelado
- **Objetivo**: Ao pressionar o atalho, congelar a tela instantaneamente e exibir o frame com uma interface HUD futurista.
- **Implementações**:
  - Captura rápida de tela do monitor primário utilizando a biblioteca Rust `xcap`.
  - Conversão em memória para PNG Data URL e transmissão instantânea via eventos IPC do Tauri (`screenshot-captured`).
  - Criação de uma moldura luminosa (*Ambient Glow HUD Frame*) com gradiente azul neon sutil (`rgba(59, 130, 246)`).
  - Fechamento imediato com a tecla **`ESC`** ou clicando no botão `✕` do topo.

---

### 🟢 Sprint 2: Ferramenta de Seleção Retangular (Marquee Box)
- **Objetivo**: Permitir ao usuário arrastar o mouse para selecionar qualquer região da tela, redimensionar e ajustar livremente.
- **Implementações**:
  - Máscara de escurecimento dinâmica via SVG (`mask-cutout`) cobrindo 55% de opacidade fora do recorte.
  - Estilo visual inspirado no **Google Lens**:
    - 4 cantoneiras angulares em formato de **"L"** (*Corner Brackets*).
    - 4 alças de borda interativas (*Edge Drag Handles*) para redimensionamento suave em qualquer direção.
  - Medidor de dimensões em tempo real (`0 × 0 px`).
  - **Barra de Ações Flutuante Inteligente**:
    - Posicionamento dinâmico que se adapta à posição do recorte na tela.
    - Algoritmo de clampeamento (*screen clamping*): caso o recorte esteja muito próximo do canto inferior ou das bordas da tela, a barra inverte automaticamente sua posição para não ser cortada fora do monitor.

---

### 🟢 Sprint 3: Busca Visual e Integração com o Google Lens
- **Objetivo**: Enviar o recorte selecionado para pesquisa visual reversa na web sem atritos.
- **Desafios e Soluções Críticas**:
  1. *Deadlock de WebView2 no Win32*:
     - **Problema**: Abrir uma janela WebView2 sincronamente dentro de um comando bloqueava o loop de mensagens do Windows.
     - **Solução**: Conversão de todos os comandos do Tauri para funções assíncronas (`async fn`).
  2. *Erro 403 Forbidden no Google Lens*:
     - **Problema**: O Google Lens bloqueava requisições POST diretas vindas de `Origin: http://localhost` ou `127.0.0.1`.
     - **Solução**: Migração para a API pública do Google Lens via URL: `https://lens.google.com/uploadbyurl?url=<image_url>`.
  3. *Sessão Anônima vs Conta Logada do Chrome*:
     - **Problema**: Envio de imagens brutas gerava um identificador anônimo (`vsrid`) que o navegador do usuário rejeitava caso estivesse logado.
     - **Solução**: Integração de upload efêmero para CDNs públicas velozes e confiáveis (`freeimage.host` / `iili.io` / `uguu.se`), permitindo que a imagem seja ingerida de forma transparente pela sessão ativa do navegador do usuário.
  4. *Janela Lateral Compacta*:
     - Janela dedicada (`430 × 720 px`) para visualização rápida da busca visual.
     - Lógica de foco: caso a janela esteja minimizada, ela é restaurada (`unminimize`), trazida para frente e recebe foco automaticamente a cada nova pesquisa.

---

### 🟢 Sprint 4: Reconhecimento Ótico de Caracteres (OCR) Nativo
- **Objetivo**: Extrair texto de qualquer imagem congelada na tela e permitir cópia seletiva no estilo Google Lens.
- **Implementações**:
  - Integração do motor nativo do Windows: **`Windows.Media.Ocr.OcrEngine`** via crate `windows 0.58`.
  - Mapeamento e renderização de **Pílulas de Texto Interativas** sobre cada palavra detectada.
  - **Recursos de Cópia Seletiva**:
    - **Clique simples**: copia a palavra específica.
    - **Arrastar o mouse (*Drag-to-select*)**: seleciona continuamente múltiplas palavras ou frases com destaque azul brilhante e copia tudo junto ao soltar o mouse.
    - **Duplo-clique**: copia a linha/sentença inteira instantaneamente.
  - Feedback visual com flash verde (`.copied`) e reset automático dos textos dos botões após 1,5s para evitar botões travados.
  - Caixa de texto de texto reconhecido no painel lateral com barra de rolagem estilizada no tema dark.

---

### 🟢 Sprint 5: Tradução na Tela e Suporte Total ao Linux
- **Objetivo**: Traduzir textos estrangeiros diretamente sobre a tela (substituindo visualmente os textos originais) e compatibilidade multiplataforma com Linux.
- **Implementações**:
  - Seletor de idioma de origem e destino na barra flutuante (com detecção automática do idioma do sistema).
  - Tradução em lote via API do Google Translate preservando as caixas delimitadoras exatas.
  - **Overlay de Tradução na Tela (*On-Screen Translation Box*)**:
    - Criação de cartões pretos translúcidos (`#1a1a1a`) posicionados exatamente sobre o texto original na tela com texto branco nítido.
    - Ajuste dinâmico de tamanho de fonte proporcional à altura e largura de cada caixa.
    - Clique em qualquer caixa traduzida copia a tradução para a área de transferência.
  - **Suporte ao Linux**:
    - Detecção automática de ambiente: Wayland vs X11 (`GDK_BACKEND`).
    - Comunicação via Unix Domain Socket (`/tmp/desklens_<uid>.sock`) para contornar restrições de atalhos globais do Wayland no GNOME.
    - Fallback para o motor Tesseract CLI no Linux com parsing de tabelas TSV.
    - Script unificado `run-linux.sh` com suporte a `--autostart` e criação de atalho `.desktop`.

---

### 🟢 Sprint 6: Acabamento de Sistema no Windows (Launcher, Hub & Tray)
- **Problema Relatado**: Ao executar o executável no Windows, abria um prompt de comando preto (CMD) que não fazia nada, o app não aparecia no menu Iniciar e o atalho parecia não funcionar.
- **Diagnóstico**:
  - No modo debug sem flags de subsistema, o executável invocava o subsistema de console do Windows.
  - Como a janela `main` estava com `visible: false`, o usuário via apenas a tela preta e, ao fechá-la, encerrava o app.
  - O ícone do System Tray no Windows exigia resolução nativa de 32×32 px.
- **Soluções Aplicadas**:
  1. Adição da flag `#![windows_subsystem = "windows"]` em `src-tauri/src/main.rs`: elimina completamente qualquer terminal CMD.
  2. Abertura do **Hub de Configurações do DeskLens** na inicialização: agora, ao abrir o programa, abre uma janela moderna (`DeskLens — Settings`) com os atalhos, status do serviço e botão "Test Screen Capture".
  3. Criação de atalhos oficiais com ícone de alta resolução:
     - Na **Área de Trabalho (`Desktop\DeskLens.lnk`)**.
     - No **Menu Iniciar (`Start Menu\Programs\DeskLens.lnk`)**.
  4. Redimensionamento do ícone de bandeja para 32×32 no Windows.

---

### 🟢 Sprint 7: Aumento Extremo da Precisão de OCR e Código (/teamwork)
- **Objetivo**: Elevar a fidelidade do OCR para blocos de código (ex: Python), símbolos especiais de sintaxe, indentação (espaços e tabs) e alinhamento milimétrico das caixas de seleção.
- **Execução Multiagente (Equipe Teamwork)**:
  - O trabalho foi conduzido em 3 rodadas de desenvolvimento e revisão adversarial com auditoria independente.
- **Melhorias Técnicas Implementadas**:
  1. **Pré-Processamento Ótico Inteligente (`src-tauri/src/ocr.rs`)**:
     - Detecção de luminância com norma **Rec.601** para identificar automaticamente editores escuros (VS Code, Google Colab, terminais).
     - Inversão de cores e estiramento de contraste (*contrast stretching*) para facilitar o trabalho dos motores óticos.
     - **Upscaling 2× com filtro Lanczos3** para que símbolos pequenos (dois-pontos `:`, vírgulas `,`, parênteses, underline `_`) sejam reconhecidos sem erros.
     - Mapeamento inverso subpixel para manter as coordenadas exatas na tela.
  2. **Normalização Sintática de Código**:
     - Remoção de caracteres de largura zero (*zero-width characters*) que quebram compiladores.
     - Normalização de aspas tipográficas (`“ ”`), símbolos de largura total e setas de código (`->`, `=>`, `===`, `<=`, `>=`).
  3. **Coalescência de Linhas e Indentação Exata**:
     - Algoritmo que resolve a fragmentação do Windows OCR, unindo palavras que pertencem à mesma linha lógica.
     - *Grid Snapping* de 2 e 4 espaços, preservando indentação original em blocos `def`, `if`, loops e comentários `#`.
     - Suporte a *hanging indent* para quebras de linha em chamadas de função longas.
  4. **Alinhamento das Caixas (*Zero-Drift*)**:
     - Substituição da borda do `.crop-box` por `outline` (eliminando um deslocamento de 1,5px).
     - Remoção de animações de zoom no hover que deslocavam as caixas sobre o texto.
- **Resultado da Auditoria**:
  - Veredito formal: **`VICTORY CONFIRMED`**.
  - **97 testes automatizados aprovados (0 falhas)** entre Rust e Node.js.

---

## 📊 3. Resumo dos Testes Automatizados

| Suíte de Testes | Quantidade | Foco |
|---|:---:|---|
| `code_ocr_tests.rs` | 16 | Reconhecimento de sintaxe Python, pontuação e indentação em temas claro/escuro |
| `tests/*.test.mjs` (Node/DOM) | 35 | Seleção por clique/arrasto, preservação de recuos, coordenadas e segurança contra injeção |
| Testes Unitários da Biblioteca (`lib.rs`) | 7 | Normalização de idiomas, codificação de URLs e contratos de BoundingBox |
| `translation_tests.rs` & `linux_compat_tests.rs` | 25 | Pipeline de tradução, enumeração de monitores e suporte a sessões Wayland/X11 |
| `adversarial_tests.rs` | 14 | Casos de borda extremos, caracteres nulos, fuzzing e fallback em falhas de rede |
| **Total Geral** | **97** | **100% de Aprovação (0 Falhas)** |

---

## 🛠️ 4. Guia Rápido de Compilação e Execução

### No Windows:
```powershell
# Execução em Modo de Desenvolvimento (Hot-reload)
npm run tauri dev

# Compilação do Binário Otimizado de Produção
cd src-tauri
cargo build --release
```
O executável final estará disponível em:
`target\release\desklens.exe`

### No Linux:
```bash
# Execução direta
./run-linux.sh

# Instalação no Menu de Aplicativos e Inicialização com o Sistema
./run-linux.sh --install
```

---

## 📝 5. Conclusão e Próximos Passos
O **DeskLens** atingiu maturidade de nível de produção:
- O atalho global `Ctrl + Shift + X` congela a tela de forma instantânea.
- A ferramenta de seleção retangular responde suavemente com controle preciso.
- A integração com Google Lens abre resultados reais no navegador padrão.
- O OCR de código reconhece estruturas de programação com fidelidade de pontuação e indentação.
- O aplicativo se integra perfeitamente ao Windows (sem telas pretas de terminal, com atalho no Menu Iniciar, Área de Trabalho e ícone de bandeja) e ao Linux.
