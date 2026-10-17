# depsandbox

Testa upgrades de dependências num sandbox isolado antes de tocar no projeto real,
e gera um relatório do que quebrou.

## Estado (MVP, fase 1)

- ✔ Adapter **npm**
- ✔ `init` (interativo, `--yes`, flags, `--dry-run`)
- ✔ `try`: snapshot → baseline 2x (deteta testes instáveis) → upgrade → comparação
- ✔ **TUI** (`depsandbox tui`): assistente de upgrade, execução ao vivo, resultados, relatórios
- ✔ Relatório em Markdown, JSON e HTML (+ `schema` com o JSON Schema)
- ☐ cargo, pip/uv · bisect · mapa de uso no código · advisories · container · `promote`

## Uso

```
cargo build --release
# binário: target/release/depsandbox(.exe)

cd o-teu-projeto-npm
depsandbox init                       # cria depsandbox.toml
depsandbox try lodash@5               # relatório em Markdown no stdout
depsandbox try axios --format json -o relatorio.json
depsandbox schema > report.schema.json
```

### TUI

```
depsandbox tui            # na pasta do projeto
```

Assistente de upgrade em 3 passos (projeto, pacote, opções), com execução ao vivo
(etapas e tempos), resultados por verificação e log. Ao abrir, a TUI consulta o registo
e sugere o primeiro pacote desatualizado; a versão alvo oferece a mais recente de cada major.

| Tecla | Ação |
|---|---|
| `↑↓` | mover entre pacote, versão, opções e botão |
| `←→` | mudar a versão alvo / alternar coluna de opções |
| `Espaço` / `Enter` | marcar opção · abrir seletor · **Iniciar Upgrade** |
| `Ctrl+K` | selecionar outra dependência (digite para filtrar) |
| `t` | iniciar o upgrade a partir de qualquer campo |
| `d` | alternar entre as etapas e os detalhes dos achados |
| `s` | guardar o relatório (md, json, html) |
| `r` | reconsultar versões novas |
| `Tab` / `1…6` | trocar de tela (Upgrade, Relatórios, Ambientes, Configurações, Documentação, Sobre) |
| `Esc` / `q` | voltar / sair |

Com “Gerar relatório” ligado, cada execução grava `.md`, `.json` e `.html` em
`.depsandbox/reports/` (a tela **Ver Relatórios** lista e abre os guardados).
**Ver Ambientes** mostra sandboxes temporários que sobraram (`d` apaga).

Requer terminal com cores 24-bit (Windows Terminal, iTerm2, GNOME Terminal...) e mínimo de 80×24;
o layout completo com barra lateral aparece a partir de 120 colunas.

Código de saída de `try`: `0` sem problemas detetados, `1` problemas, `2` erro da ferramenta.

## Estrutura

```
crates/core      modelo do relatório, trait Adapter, sandbox, motor
crates/adapters  um adapter por ecossistema (npm)
crates/report    vistas do relatório (md, json)
crates/tui       interface interativa (ratatui + crossterm)
crates/cli       clap + wizard de init
```

Novo ecossistema = implementar `depsandbox_core::Adapter` em `crates/adapters`.

## Notas

- O projeto real nunca é modificado: `try` trabalha numa cópia em diretório temporário
  (ignora `node_modules`, `target`, `.venv`, `.git`).
- "Passou" significa só que nenhuma verificação detetou problema.
- Scripts de `postinstall` correm no sandbox sem isolamento forte; o modo container
  está previsto para uma fase seguinte.
- O `Cargo.lock` está incluído de propósito (é uma aplicação).
