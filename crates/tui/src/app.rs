//! Estado da TUI. Sem I/O de terminal: tudo aqui é testável.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use depsandbox_core::{Dependency, Report, UpgradeTarget, Verdict};
use ratatui::widgets::ListState;
use std::collections::HashMap;

pub enum RowState {
    Idle,
    Queued,
    Running(String),
    Done(Box<Report>),
    Failed(String),
}

pub struct Row {
    pub dep: Dependency,
    pub marked: bool,
    pub ignored: bool,
    pub state: RowState,
}

/// Mensagens vindas das threads de trabalho.
pub enum Msg {
    Latest(Result<HashMap<String, String>, String>),
    Progress(usize, String),
    Done(usize, Result<Report, String>),
    Finished,
}

/// Efeitos que o ciclo principal tem de executar.
#[derive(Debug)]
pub enum Cmd {
    Run(Vec<(usize, UpgradeTarget)>),
    RefreshLatest,
    Save(usize),
}

pub struct App {
    pub project: String,
    pub rows: Vec<Row>,
    pub list: ListState,
    pub scroll: u16,
    pub latest_loading: bool,
    pub running: bool,
    pub status: String,
    pub should_quit: bool,
    pub tick: usize,
    quit_armed: bool,
}

impl App {
    pub fn new(project: String, deps: Vec<Dependency>, ignore: &[String]) -> Self {
        let rows: Vec<Row> = deps
            .into_iter()
            .map(|dep| Row {
                ignored: ignore.contains(&dep.name),
                dep,
                marked: false,
                state: RowState::Idle,
            })
            .collect();
        let mut list = ListState::default();
        if !rows.is_empty() {
            list.select(Some(0));
        }
        Self {
            project,
            rows,
            list,
            scroll: 0,
            latest_loading: false,
            running: false,
            status: String::new(),
            should_quit: false,
            tick: 0,
            quit_armed: false,
        }
    }

    pub fn cursor(&self) -> usize {
        self.list.selected().unwrap_or(0)
    }

    pub fn current_row(&self) -> Option<&Row> {
        self.rows.get(self.cursor())
    }

    fn move_to(&mut self, i: usize) {
        if !self.rows.is_empty() {
            self.list.select(Some(i.min(self.rows.len() - 1)));
            self.scroll = 0;
        }
    }

    pub fn on_msg(&mut self, m: Msg) {
        match m {
            Msg::Latest(Ok(map)) => {
                for r in &mut self.rows {
                    r.dep.latest = map.get(&r.dep.name).cloned();
                }
                self.latest_loading = false;
                let n = self.rows.iter().filter(|r| r.dep.latest.is_some()).count();
                self.status = format!("{n} pacote(s) com versão nova");
            }
            Msg::Latest(Err(e)) => {
                self.latest_loading = false;
                self.status = format!("não foi possível consultar versões: {}", first_line(&e));
            }
            Msg::Progress(i, text) => {
                if let Some(r) = self.rows.get_mut(i) {
                    r.state = RowState::Running(text);
                }
            }
            Msg::Done(i, res) => {
                if let Some(r) = self.rows.get_mut(i) {
                    r.state = match res {
                        Ok(rep) => RowState::Done(Box::new(rep)),
                        Err(e) => RowState::Failed(e),
                    };
                }
            }
            Msg::Finished => {
                self.running = false;
                for r in &mut self.rows {
                    r.marked = false;
                }
                let bad = self
                    .rows
                    .iter()
                    .filter(|r| matches!(&r.state, RowState::Done(rep) if rep.verdict != Verdict::NoIssuesDetected))
                    .count();
                self.status = if bad == 0 {
                    "testes concluídos: nenhum problema detectado".into()
                } else {
                    format!("testes concluídos: {bad} upgrade(s) com problemas")
                };
            }
        }
    }

    pub fn on_key(&mut self, k: KeyEvent) -> Option<Cmd> {
        let ctrl_c = k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c');
        if ctrl_c || matches!(k.code, KeyCode::Char('q') | KeyCode::Esc) {
            if self.running && !self.quit_armed {
                self.quit_armed = true;
                self.status = "teste em curso: prima q outra vez para sair (pode ficar lixo em /tmp)".into();
            } else {
                self.should_quit = true;
            }
            return None;
        }
        self.quit_armed = false;

        let cur = self.cursor();
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_to(cur.saturating_sub(1)),
            KeyCode::Down | KeyCode::Char('j') => self.move_to(cur + 1),
            KeyCode::Home | KeyCode::Char('g') => self.move_to(0),
            KeyCode::End | KeyCode::Char('G') => self.move_to(usize::MAX / 2),
            KeyCode::PageDown => self.scroll = self.scroll.saturating_add(10),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(10),
            KeyCode::Char(' ') => {
                if let Some(r) = self.rows.get_mut(cur) {
                    if r.ignored {
                        self.status = format!("'{}' está em `ignore` no depsandbox.toml", r.dep.name);
                    } else {
                        r.marked = !r.marked;
                    }
                }
            }
            KeyCode::Char('a') => {
                let all = self.rows.iter().filter(|r| r.dep.latest.is_some() && !r.ignored).all(|r| r.marked);
                for r in self.rows.iter_mut().filter(|r| r.dep.latest.is_some() && !r.ignored) {
                    r.marked = !all;
                }
            }
            KeyCode::Char('r') => {
                if self.latest_loading {
                    return None;
                }
                self.latest_loading = true;
                self.status = "a consultar o registo...".into();
                return Some(Cmd::RefreshLatest);
            }
            KeyCode::Char('s') => {
                return match self.rows.get(cur).map(|r| &r.state) {
                    Some(RowState::Done(_)) => Some(Cmd::Save(cur)),
                    _ => {
                        self.status = "nada para guardar: teste este upgrade primeiro (t)".into();
                        None
                    }
                };
            }
            KeyCode::Char('t') | KeyCode::Enter => return self.start_run(cur),
            _ => {}
        }
        None
    }

    fn start_run(&mut self, cur: usize) -> Option<Cmd> {
        if self.running {
            self.status = "já há testes em execução".into();
            return None;
        }
        let mut idx: Vec<usize> =
            self.rows.iter().enumerate().filter(|(_, r)| r.marked && !r.ignored).map(|(i, _)| i).collect();
        if idx.is_empty() {
            match self.rows.get(cur) {
                Some(r) if !r.ignored => idx.push(cur),
                Some(r) => {
                    self.status = format!("'{}' está em `ignore`", r.dep.name);
                    return None;
                }
                None => return None,
            }
        }
        let jobs: Vec<(usize, UpgradeTarget)> = idx
            .into_iter()
            .map(|i| {
                let r = &mut self.rows[i];
                r.state = RowState::Queued;
                (i, UpgradeTarget { package: r.dep.name.clone(), version: r.dep.latest.clone() })
            })
            .collect();
        self.running = true;
        self.status = format!("a testar {} upgrade(s)...", jobs.len());
        Some(Cmd::Run(jobs))
    }

    /// Texto do relatório do item selecionado, para guardar em ficheiro.
    pub fn report_of(&self, i: usize) -> Option<&Report> {
        match &self.rows.get(i)?.state {
            RowState::Done(r) => Some(r),
            _ => None,
        }
    }
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventKind;

    fn key(c: char) -> KeyEvent {
        let mut k = KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        k.kind = KeyEventKind::Press;
        k
    }

    fn dep(name: &str) -> Dependency {
        Dependency { name: name.into(), current: "^1.0.0".into(), latest: None, dev: false }
    }

    fn app() -> App {
        App::new("demo".into(), vec![dep("a"), dep("b"), dep("c")], &["c".to_string()])
    }

    #[test]
    fn latest_then_mark_all_then_run() {
        let mut a = app();
        a.on_msg(Msg::Latest(Ok(HashMap::from([("a".into(), "2.0.0".into()), ("c".into(), "3.0.0".into())]))));
        assert_eq!(a.rows[0].dep.latest.as_deref(), Some("2.0.0"));
        assert!(a.rows[1].dep.latest.is_none());

        a.on_key(key('a'));
        // só `a` é marcado: `b` está atualizado e `c` é ignorado
        assert!(a.rows[0].marked && !a.rows[1].marked && !a.rows[2].marked);

        let cmd = a.on_key(key('t')).unwrap();
        match cmd {
            Cmd::Run(jobs) => {
                assert_eq!(jobs.len(), 1);
                assert_eq!(jobs[0].1.spec(), "a@2.0.0");
            }
            _ => panic!("esperava Run"),
        }
        assert!(a.running);
        assert!(a.on_key(key('t')).is_none(), "não deve iniciar segunda execução");
    }

    #[test]
    fn run_without_marks_uses_cursor_and_ignored_is_refused() {
        let mut a = app();
        a.on_key(key('j'));
        let Some(Cmd::Run(jobs)) = a.on_key(key('t')) else { panic!() };
        assert_eq!(jobs[0].0, 1);
        assert_eq!(jobs[0].1.spec(), "b@latest");
        a.on_msg(Msg::Finished);

        a.on_key(key('G'));
        assert_eq!(a.cursor(), 2);
        assert!(a.on_key(key('t')).is_none());
        assert!(a.status.contains("ignore"));
    }

    #[test]
    fn quit_needs_confirmation_while_running() {
        let mut a = app();
        a.on_key(key('t'));
        a.on_key(key('q'));
        assert!(!a.should_quit);
        a.on_key(key('q'));
        assert!(a.should_quit);
    }
}
