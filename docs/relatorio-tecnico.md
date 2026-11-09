# DeskLens
## Relatório técnico e arquitetural

**Edição:** 9 de novembro de 2026 | **Projeto:** Gustavo Geminiano

**Base técnica:** código e configuração do commit `5d480f6`. Este documento consolida o estado do projeto após as alterações de OCR, tradução, busca visual, interface e organização do repositório. Não presume que uma release pública contenha todas essas mudanças.

### Resumo executivo

O DeskLens é um utilitário desktop para Windows e Linux. Captura uma região da tela, reconhece seu texto, permite copiá-lo, traduz o conteúdo sobre a imagem e inicia buscas visuais pelo navegador. A interação combina uma janela de configurações, bandeja do sistema e um overlay de seleção.

O projeto usa Rust e Tauri v2 no backend e HTML, CSS e JavaScript na interface. Há dois caminhos de OCR: o nativo, padrão do código-fonte, e o RapidOCR/ONNX, habilitado explicitamente pela feature `ocr-v2`. O executável local recompilado nesta sessão usa a configuração experimental.

### O que muda em relação ao relatório anterior

| Informação anterior | Situação correta nesta edição |
| --- | --- |
| RAM de 18 a 25 MB e executável menor que 20 MB | Não há medição atual que sustente esses valores. O runtime OCR empacotado acrescenta tamanho e processos. |
| Apenas WinRT/Tesseract | Há também um worker persistente RapidOCR/ONNX, experimental e offline. |
| Busca visual centrada na janela embutida | O botão Search Image abre Google Lens no navegador padrão; o painel de resultados também permanece implementado. |
| Preservação sintática de código como garantia | OCR pode errar. A nova reconstrução preserva a transcrição literal e não corrige código por regras sintáticas. |
| Maturidade completa de produção | Existem regressões e falhas conhecidas; a promoção global do novo motor continua condicionada aos critérios de qualidade. |

**Leitura do relatório:** funcionalidades e fluxo; arquitetura; tecnologias; OCR; árvore de arquivos; construção e testes; medições; limites e referências.

<!-- page -->
# 1. Funcionalidades e experiência de uso

### Captura e seleção

O atalho `Ctrl+Shift+X` ou a ação da bandeja inicia a captura. Uma imagem estática é apresentada em tela cheia, com uma máscara de escurecimento e seleção retangular ajustável pelas bordas. As coordenadas visuais são convertidas para pixels da imagem original.

O recorte recebe um identificador de sessão. Respostas antigas de reconhecimento e tradução são descartadas quando a seleção, o modo ou o idioma muda. A captura de origem antecede a apresentação do overlay, evitando reconhecer a própria interface durante o fluxo normal.

### Texto e tradução

O reconhecimento alimenta destaques selecionáveis, cópia do texto integral e seleção de trechos. No caminho experimental, posições de texto usam intervalos UTF-16, compatíveis com JavaScript, inclusive para caracteres fora do plano básico Unicode.

A tradução recebe linhas completas e agrupa trechos em parágrafos quando adequado. Isso substitui o comportamento inadequado de traduzir cada palavra em uma caixa independente. As caixas traduzidas são medidas no navegador: a fonte diminui até um mínimo legível; se o conteúdo ainda não couber, a altura disponível é usada e a rolagem mantém o texto acessível.

| Controle atual | Comportamento |
| --- | --- |
| Search Image | Preserva o recorte, oculta o overlay e inicia a busca visual. |
| Translate | Traduz para o idioma escolhido e exibe blocos sobre a imagem. |
| Auto / Text / Code | Seleciona o modo de reconstrução do OCR experimental; o motor nativo mantém seu pipeline existente. |
| Copy Text / Copy Translation | Copia o resultado correspondente à sessão ativa. |
| Review recognized text | Mostra a transcrição em uma área somente de leitura. |
| Retry / Copy anyway | Permite nova leitura ou cópia explícita de texto marcado como incerto. |
| Move menu | Alça para arrastar o menu, sem mover o recorte nem alterar a tradução. |
| Esc / fechar overlay | Encerra a seleção e invalida os resultados pendentes. |

O menu mantém a posição manual durante atualizações de OCR e tradução, respeita os limites da tela e volta ao posicionamento automático em uma nova seleção. Os botões Clear e Test Screen Capture foram removidos. Na janela de configurações permanece o botão azul Hide to System Tray.

<!-- page -->
# 2. Arquitetura e circulação de dados

### Camadas principais

| Camada | Responsabilidade |
| --- | --- |
| Interface WebView | Seleção, estado visual, idiomas, revisão, destaques e menu flutuante. |
| Tauri e Rust | Comandos IPC, eventos, janelas, captura, clipboard, rede e integração com o desktop. |
| Serviço de OCR | Sessões imutáveis de recorte, cache por modo e controle do processo experimental. |
| Motores de reconhecimento | Windows OCR/Tesseract no padrão; RapidOCR/ONNX no build experimental. |
| Serviços externos | Tradução de texto e hospedagem temporária de imagem para a busca visual. |

### Fluxo operacional

1. Uma ação da bandeja, atalho ou integração Linux solicita a captura ao backend.
2. Rust captura a tela e fornece a imagem para o overlay.
3. A seleção cria um recorte com identificador, conteúdo e dimensões coerentes.
4. O frontend solicita o OCR; consumidores simultâneos compartilham a mesma operação e resultado por recorte e modo.
5. A resposta canônica alimenta leitura, cópia e tradução, com guardas contra respostas obsoletas.
6. Na tradução, linhas são agrupadas no backend; o frontend aplica escala e ajusta as caixas à renderização real.
7. Na busca visual, a imagem é salva antes dos eventos que limpam a seleção. O upload e a abertura do navegador usam essa cópia.

### Janelas e integração com o desktop

A janela `main` alterna entre configurações e overlay. A janela `results` oferece um painel auxiliar de busca visual. O menu da bandeja contém captura, configurações e encerramento. A interface não depende do botão de captura de teste que existia nas configurações.

No Linux, há um socket Unix local e a opção `--trigger`, além do launcher `run-linux.sh`. O script pode configurar o atalho no GNOME e o ambiente de compatibilidade. Isso ajuda na integração com o desktop, mas não elimina todas as restrições impostas pelo Wayland.

A API de comandos e os eventos do Tauri ligam as duas camadas. Identificadores de sessão impedem que uma leitura lenta substitua um recorte mais recente; a limpeza de respostas tardias é condicional ao identificador do recorte.

<!-- page -->
# 3. Tecnologias e dependências

As versões de crates abaixo são as faixas declaradas nos manifestos; a resolução exata do app fica no Cargo.lock da raiz. As versões Python citadas são fixadas nos arquivos de requisitos.

| Tecnologia | Papel no projeto |
| --- | --- |
| Rust, edição 2021 | Backend, processamento de imagens, estado, processos e integrações nativas. |
| Tauri v2 e tauri-build | Aplicação desktop, IPC, eventos, WebViews, bandeja e empacotamento. |
| tauri-plugin-global-shortcut v2 | Registro do atalho global. |
| HTML5, CSS3 e JavaScript | Interface sem framework frontend; manipulação de DOM, seleção e overlays. |
| WebKitGTK / WebView2 | Renderização da interface em Linux / Windows. |
| xcap 0.9 e image 0.25 | Captura e transformação de imagens; codificação PNG. |
| windows 0.58 / Windows.Media.Ocr | OCR e integrações WinRT no Windows. |
| Tesseract | OCR nativo do Linux, com pacotes de idiomas instalados no sistema. |
| serde, serde_json, base64 | Estruturas IPC, JSON e transporte de imagens em data URLs. |
| arboard 3.4, libc 0.2, tempfile 3 | Clipboard, integração Unix e arquivos temporários. |
| reqwest 0.13 | Chamadas HTTP para tradução; o fluxo de hospedagem de imagens também usa comandos do sistema. |
| RapidOCR 3.10.0 / ONNX Runtime 1.31.1 | Inferência experimental em CPU, sem CUDA ou DirectML. |
| NumPy, OpenCV headless e Pillow | Matrizes, transformações de pixels, geometria e imagens de teste. |
| PyInstaller 6.22.3 | Runtime Python congelado em diretório, distribuído junto ao app experimental. |
| Node.js 20 / npm / node:test | Ferramentas de desenvolvimento e testes do frontend. |
| Python 3.12 | Ambiente de construção e diagnóstico do OCR experimental. |
| GitHub Actions | Validação nativa por plataforma, relatórios e instaladores. |

### Modelos do OCR experimental

O detector é `ch_PP-OCRv5_det_mobile.onnx` e o reconhecedor é `latin_PP-OCRv5_rec_mobile.onnx`. A escolha do detector não significa que a transcrição use um reconhecedor chinês: a etapa de reconhecimento usa o modelo latino.

Nomes e hashes SHA-256 são fixados no adaptador de benchmark. O download ocorre na preparação explícita de build. A execução normal verifica os modelos locais e falha se estiverem ausentes ou inválidos. O usuário final recebe o runtime e os modelos juntos, sem instalar Python separadamente.

<!-- page -->
# 4. OCR: motores, sessões e fidelidade

### Caminho padrão e caminho experimental

**Padrão do repositório:** Windows OCR no Windows e Tesseract no Linux. O módulo nativo mantém preprocessamento, agrupamento e heurísticas históricas, inclusive tratamento de código. Seus resultados não devem ser confundidos com a política literal do novo motor.

**Feature ocr-v2:** um processo persistente RapidOCR carrega os modelos uma vez, recebe requisições JSON por stdin e devolve respostas por stdout. Logs ficam em stderr. A comunicação força UTF-8 também no Windows, evitando problemas com acentos e caminhos de arquivo.

### Etapas do worker experimental

1. Detectar fragmentos e reconhecer texto com caixas de caracteres.
2. Agrupar fragmentos por posição vertical, linha visual e separação de colunas.
3. Reconhecer linhas inteiras para recuperar conteúdo perdido pela fragmentação.
4. Em até três linhas com confiança inferior a 0,90, permitir duas variantes de pixels: ampliação cúbica de 150% e binarização Otsu. Uma leitura diferente só é escolhida com ganho de confiança superior a 0,02 no mesmo modelo.
5. Preservar a geometria quando as leituras concordam nos caracteres; inferir o recuo de código por evidência espacial e passo monoespaçado.
6. Ajustar regiões de caracteres ao contraste observado dentro das caixas CTC e preservar os polígonos originais como evidência.
7. Emitir texto, linhas, palavras, blocos, intervalos UTF-16, coordenadas e incertezas.

Esse processo não insere operadores por um parser de Python nem transforma código inválido em código válido. Ainda pode produzir transcrições incorretas; leituras conflitantes ficam disponíveis para revisão.

### Serviço Rust e ciclo de vida

O serviço guarda uma imagem imutável e seu identificador. Um cache por modo compartilha operações simultâneas e reutiliza resultados. Falhas podem ser tentadas novamente; respostas para seleções antigas são rejeitadas.

A inicialização do processo tem limite de 30 segundos e cada requisição, 15 segundos. O serviço permite uma reinicialização após falha. Um build experimental não troca silenciosamente para o motor nativo se o worker estiver ausente ou falhar.

O diretório completo `ocr-runtime`, com executável, `_internal` e modelos, integra os recursos Tauri. Copiar apenas o executável não produz uma instalação funcional.

<!-- page -->
# 5. Estrutura do repositório e funções

| Caminho | Conteúdo e responsabilidade |
| --- | --- |
| src/ | Interface: main.js, index.html, styles.css e painel results.*. |
| src-tauri/src/lib.rs | Comandos Tauri, captura, clipboard, integração Linux, janelas e busca visual. |
| src-tauri/src/main.rs | Entrada do executável e encaminhamento de --trigger. |
| src-tauri/src/ocr.rs | Motores nativos e preprocessamento histórico de OCR. |
| src-tauri/src/ocr_service.rs | Sessão de recorte, cache, comunicação e ciclo de vida do worker. |
| src-tauri/src/translation.rs | Provedores de tradução e agrupamento de linhas em parágrafos. |
| src-tauri/tests/ | Regressões e diagnósticos Rust, incluindo integração OCR. |
| src-tauri/capabilities/ e icons/ | Permissões Tauri e ícones de distribuição. |
| tests/ e tests/fixtures/ | Regressões frontend e imagens compartilhadas; synthetic/ guarda exemplos gerados. |
| scripts/ocr_benchmark/ | Modelos fixados, anotações, comparação de estágios, métricas e relatório HTML. |
| scripts/generate_test_image.ps1 | Geração Windows das imagens sintéticas versionadas. |
| tools/ocr_worker/ | Inferência, reconstrução, protocolo, corpus, empacotamento e smoke tests. |
| tools/ocr-diagnostic/ | Workspace Cargo independente para o diagnóstico nativo sem a aplicação gráfica. |
| docs/ | Guias atuais, relatórios arquivados e pesquisas de OCR datadas. |
| .github/workflows/ | Validação nativa, builds experimentais e releases. |

### Comandos e pontos de entrada relevantes

`trigger_capture` obtém a captura; `crop_selection` cria o recorte; `get_latest_crop` consulta sua imagem. `extract_text_from_crop` fornece OCR canônico e `copy_text_from_crop` usa o resultado da sessão. `clear_crop_session` invalida somente a sessão indicada.

`search_google_lens` prepara o upload e abre o navegador; `open_visual_search` abre o painel auxiliar; `open_search_provider` oferece as alternativas de busca implementadas. `save_crop_to_disk`, `copy_crop_to_clipboard` e `copy_text_to_clipboard` tratam exportação e clipboard. `translate_on_screen` executa a tradução dos blocos.

No frontend, `requestBackendCrop`, `getCanonicalOcr`, `renderTextHighlights`, `renderTranslationOverlays` e `positionActionToolbar` coordenam recorte, leitura e apresentação. A função interna de limpeza continua existindo apesar da remoção do botão Clear.

O Cargo.lock da raiz controla o app. O diagnóstico independente possui seu próprio lockfile. O lock duplicado em src-tauri e o falso arquivo ICNS foram removidos. Ambientes, modelos baixados, builds e relatórios locais não são versionados.

<!-- page -->
# 6. Tradução, busca visual e uso da rede

### Tradução

O frontend envia o texto das linhas completas, e o backend agrupa linhas adjacentes em parágrafos quando apropriado. A resposta inclui texto original, tradução, geometria e idioma detectado. A posição é convertida dos pixels do recorte para o overlay, sem uma caixa separada por palavra.

O código oferece tradução via Google e fallback MyMemory, além de integrações opcionais DeepL e Gemini por variáveis de ambiente. A seleção atual verifica DEEPL_API_KEY antes de GEMINI_API_KEY. Chaves e limites dos provedores não são distribuídos como parte do código.

Os serviços externos podem falhar, mudar ou limitar requisições. Sua presença no código não equivale a uma garantia atual de disponibilidade. A troca inteligente de idioma pode escolher outro destino quando a origem detectada coincide com o idioma de destino.

### Busca visual

O botão Search Image usa o recorte ativo. O backend salva a imagem antes de ocultar o overlay e emitir eventos que limpam a seleção. Essa ordem corrige a falha em que a busca perdia a própria imagem de entrada.

O fluxo implementado tenta hospedar a imagem em uguu.se, kappa.lol e freeimage.host. Com uma URL de imagem, abre Google Lens no navegador padrão. Se a hospedagem falhar, há um caminho de fallback para Google Images, com tentativa de disponibilizar a imagem no clipboard.

O painel results.* permanece no projeto para busca visual auxiliar. Portanto, há código de janela secundária, mas é incorreto descrever o botão principal atual como uma busca obrigatoriamente embutida no app.

| Operação | Dados e conectividade |
| --- | --- |
| Captura, recorte e OCR | Processamento local; OCR experimental normal não baixa modelos. |
| Tradução | Texto reconhecido é enviado ao provedor selecionado ou ao fallback. |
| Search Image | O recorte é enviado a um serviço de hospedagem e sua URL é encaminhada à busca visual. |
| Preparação do worker | Dependências e modelos são obtidos na etapa explícita de construção. |

A separação entre OCR local e ações online deve permanecer clara na documentação e na experiência de uso. Não há justificativa para descrever a aplicação inteira como offline.

<!-- page -->
# 7. Construção, distribuição e verificação

### Comandos de desenvolvimento

Os comandos partem da raiz do repositório. O ambiente deve conter Node.js 20, Rust estável e as bibliotecas nativas exigidas pelo Tauri e pela captura.

```text
npm ci
npm run tauri dev
npm run tauri build
npm test
cargo check --locked -p desklens
```

No Linux, `run-linux.sh --dev` configura o ambiente auxiliar e executa o modo de desenvolvimento. O launcher comum prefere um binário release já existente; alterações exigem recompilar e reiniciar. O script de atalho usa agora o caminho real do checkout, sem um diretório pessoal fixo.

Para OCR experimental, preparar o ambiente Python, os requisitos fixados e os modelos verificados antes de empacotar:

```text
npm run ocr:build -- --bundles deb
npm run ocr:build -- --bundles nsis
```

O primeiro comando destina-se ao Linux; o segundo, ao Windows. Binaries e instaladores do workspace ficam em `target/release/` e `target/release/bundle/`. O pipeline de release por tags é separado dos builds experimentais; uma compilação local não publica automaticamente uma release.

### O que os testes verificam

- Frontend: seleção, sessões antigas, clipboard, idioma, tradução, arraste do menu e cancelamento.
- Navegador real: dimensões de caixas, quebra de linhas e acesso ao texto longo; o teste informa skip se não houver Chrome/Chromium compatível no ambiente.
- Rust: preprocessamento, OCR nativo, geometria, integração e cache concorrente do serviço de sessão.
- Python: protocolo do worker, UTF-8, layout, corpus, geometria e avaliação.
- Smoke instalado: comunicação do app com o runtime empacotado, cache, texto literal e ambiente sem Python/Tesseract no PATH do processo filho.

**Verificação recente da interface:** 254 testes passaram na remoção do Clear. A compilação local release também terminou com sucesso. Esses números não representam toda a suíte Rust nem uma nova execução Windows do commit atual.

Nos testes Rust afetados pela reorganização, passaram 26, houve um ignorado e reapareceram duas falhas de OCR conhecidas. Uma execução nativa completa anterior registrou, por plataforma, 241 passados, cinco falhos e um ignorado. As falhas não foram convertidas em sucesso nem removidas para melhorar o indicador.

<!-- page -->
# 8. Evidências, limitações e evolução

### Comparação histórica documentada

Os valores abaixo vêm da validação nativa do commit `7c654fc`, em 8 de novembro de 2026. Não são uma nova medição do commit de referência deste relatório. A avaliação usa 20 casos sintéticos de desenvolvimento e dez sintéticos de conteúdo separado, além da captura do usuário com overlay.

| Plataforma / conjunto | CER pipeline nativo | CER OCR V2 | Critérios por família |
| --- | --- | --- | --- |
| Linux / desenvolvimento | 16,59% | 0,87% | Não atendidos |
| Windows / desenvolvimento | 17,62% | 0,48% | Não atendidos |
| Linux / separado | 26,62% | 0,83% | Não atendidos |
| Windows / separado | 28,62% | 0,67% | Atendidos |

CER é a distância de edição por caractere, normalizada pelo texto esperado, incluindo espaços nesta tabela. Ganho agregado não exclui regressões em uma categoria. O corpus sintético não substitui capturas reais limpas e independentes.

No smoke histórico empacotado da captura obrigatória, a mediana de cinco execuções aquecidas foi 516,58 ms no Linux e 799,42 ms no Windows. As inicializações foram 722,26 ms e 1.097,89 ms. São medições dos runners daquele experimento, não promessas de latência para qualquer computador.

Os instaladores daquele commit foram executados em Linux e Windows, com rede bloqueada e PATH do processo filho vazio. A revisão posterior adicionou releituras de pixels e teste das escalas 100%, 125%, 150% e 200%; há evidência local de acerto literal dessas escalas. Isso não autoriza atribuir os tempos ou resultados históricos, sem nova execução, ao código mais recente.

### Limites e prioridades

Manter o motor novo opt-in enquanto houver regressões por família; ampliar o conjunto de capturas reais limpas; repetir validação nativa por plataforma após mudanças relevantes; corrigir as falhas conhecidas sem alterar referências para esconder erros; e medir memória, disco e latência do app completo por variante de build.

O tamanho e a memória de um processo Rust isolado não incluem automaticamente WebViews, worker Python congelado e modelos ONNX. Assim, foram retiradas as garantias de 18-25 MB de RAM, binário menor que 20 MB, latência zero e maturidade irrestrita de produção do relatório anterior.

### Referências técnicas verificáveis

Código-base: github.com/GustavoGeminiano/desklens, commit 5d480f6. Guias: README.md; docs/architecture.md; docs/development.md; docs/testing.md. Implementação: src-tauri/src/{lib,ocr,ocr_service,translation}.rs e src/main.js. Runtime: tools/ocr_worker/README.md e scripts/ocr_benchmark/rapidocr_engine.py.

Medições e decisões: docs/research/ocr/10-final-validation.md; 11-offline-canonical-runtime.md; 12-native-installers-and-scale-recovery.md; 12-native-v2-results.json; 13-validation-race.md. CI histórico: actions/runs/42499197612. O PDF anterior foi usado como referência editorial, não como autoridade sobre o estado atual do código.
