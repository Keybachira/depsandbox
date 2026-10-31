//! Interface gráfica do depsandbox construída com GPUI.
//!
//! O motor continua em `depsandbox-core`; este crate só transforma o estado
//! do projecto numa janela desktop. Isso permite manter a TUI Ratatui durante
//! a migração e trocar as duas interfaces sem duplicar a lógica de sandbox.

use anyhow::{Context as AnyhowContext, Result};
use depsandbox_core::{Adapter, Config, Dependency};
use gpui::{
    div, prelude::*, px, rgb, size, App, Application, Bounds, Context as GpuiContext, SharedString, Window,
    WindowBounds, WindowOptions,
};
use std::path::PathBuf;
use std::sync::Arc;

struct DepsandboxView {
    project: SharedString,
    ecosystem: SharedString,
    dependencies: Vec<DependencyRow>,
}

struct DependencyRow {
    name: SharedString,
    current: SharedString,
    kind: SharedString,
}

impl Render for DepsandboxView {
    fn render(&mut self, _window: &mut Window, _cx: &mut GpuiContext<Self>) -> impl IntoElement {
        let rows = self.dependencies.iter().map(|dependency| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .border_b_1()
                .border_color(rgb(0x19364a))
                .py_2()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(dependency.name.clone())
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0x7893a5))
                                .child(dependency.kind.clone()),
                        ),
                )
                .child(
                    div()
                        .text_color(rgb(0xb8f23a))
                        .child(dependency.current.clone()),
                )
        });

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x06121f))
            .text_color(rgb(0xd6e2eb))
            .p_6()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_2xl().text_color(rgb(0xb8f23a)).child("depsandbox"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x7893a5))
                            .child(format!("{} · {}", self.project, self.ecosystem)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .bg(rgb(0x0b2133))
                    .border_1()
                    .border_color(rgb(0x184e70))
                    .rounded_md()
                    .p_4()
                    .gap_2()
                    .child(div().text_lg().child("Dependências"))
                    .children(rows),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x7893a5))
                    .child("Selecione uma dependência para iniciar um upgrade · GUI GPUI"),
            )
    }
}

/// Abre a interface gráfica GPUI para o projecto indicado.
pub fn run(project: PathBuf, adapter: Arc<dyn Adapter>, _cfg: Config) -> Result<()> {
    let dependencies = adapter
        .dependencies(&project)
        .with_context(|| format!("a ler dependências de {}", project.display()))?;
    if dependencies.is_empty() {
        anyhow::bail!("nenhuma dependência encontrada em {}", project.display());
    }

    let project_name = project
        .canonicalize()
        .ok()
        .and_then(|p| p.file_name().map(|name| name.to_string_lossy().into_owned()))
        .unwrap_or_else(|| project.display().to_string());
    let ecosystem = adapter.ecosystem().name().to_string();
    let dependency_rows = dependencies.into_iter().map(row).collect::<Vec<_>>();

    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(900.), px(620.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| DepsandboxView {
                    project: project_name.into(),
                    ecosystem: ecosystem.into(),
                    dependencies: dependency_rows,
                })
            },
        )
        .expect("a abrir a janela GPUI");
    });
    Ok(())
}

fn row(dependency: Dependency) -> DependencyRow {
    DependencyRow {
        name: dependency.name.into(),
        current: dependency.current.into(),
        kind: if dependency.dev { "dev" } else { "runtime" }.into(),
    }
}
