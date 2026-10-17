# depsandbox

Testa upgrades de dependências num sandbox isolado antes de tocar no projeto real,
e gera um relatório do que quebrou.

## Estado (MVP, fase 1)

- ✔ Adapter **npm**
- ✔ `init` (interativo, `--yes`, flags, `--dry-run`)
- ✔ `try`: snapshot → baseline 2x (deteta testes instáveis) → upgrade → comparação
- ✔ **TUI** (`depsandbox tui`): lista dependências, marca, testa em segundo plano, vê o relatório
- ✔ Relatório em Markdown e JSON (+ `schema` com o JSON Schema)
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

| Tecla | Ação |
|---|---|
| `↑↓` / `j k` | mover |
| `espaço` | marcar/desmarcar pacote |
| `a` | marcar/desmarcar todos os desatualizados |
| `t` / `Enter` | testar os marcados (ou o selecionado, se nenhum marcado) |
| `s` | guardar o relatório do selecionado (`depsandbox-report-<pacote>.md`) |
| `r` | reconsultar versões novas |
| `PgUp/PgDn` | rolar o detalhe |
| `q` / `Esc` | sair |

Os testes correm em fila, um sandbox de cada vez. Pacotes em `ignore` aparecem a cinzento e não são testados.

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
