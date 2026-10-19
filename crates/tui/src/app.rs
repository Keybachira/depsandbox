//! Estado da TUI. Sem I/O de terminal: tudo aqui é testável.
use crate::export::{self, Saved, SavedReport};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use depsandbox_core::engine::RunOptions;
use depsandbox_core::versions::{clean_spec, major_choices};
use depsandbox_core::{Config, Dependency, Report, UpgradeTarget, Verdict};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Upgrade,
    Reports,
    Envs,
    Settings,
    Docs,
    About,
}

impl Screen {
    pub const ALL: [Screen; 6] =
        [Screen::Upgrade, Screen::Reports, Screen::Envs, Screen::Settings, Screen::Docs, Screen::About];

    pub fn name(self) -> &'static str {
        match self {
            Screen::Upgrade => "Executar Upgrade",
            Screen::Reports => "Ver Relatórios",
            Screen::Envs => "Ver Ambientes",
            Screen::Settings => "Configurações",
            Screen::Docs => "Documentação",
            Screen::About => "Sobre",
        }
    }
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
    fn step(self, delta: isize) -> Screen {
        let n = Self::ALL.len() as isize;
        Self::ALL[(self.index() as isize + delta).rem_euclid(n) as usize]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    Pkg,
    Ver,
    /// 0 testes · 1 tipos · 2 build · 3 relatório
    Opt(usize),
    Start,
}

#[derive(Clone, Debug)]
pub struct VersionChoice {
    pub label: String,
    /// `None` = tag `latest` do registo.
    pub version: Option<String>,
    pub is_latest: bool,
}

pub struct LiveStep {
    pub name: String,
    pub started: Instant,
    pub duration: Option<Duration>,
}

pub enum RunState {
    Idle,
    Running { started: Instant, steps: Vec<LiveStep> },
    Done { report: Box<Report>, saved: Option<Result<Saved, String>> },
    Failed(String),
}

pub struct EnvInfo {
    pub tools: Vec<(String, String)>,
    pub branch: Option<String>,
}

pub struct DoneInfo {
    pub report: Box<Report>,
    pub saved: Option<Result<Saved, String>>,
}

/// Mensagens vindas das threads de trabalho.
pub enum Msg {
    Latest(Result<HashMap<String, String>, String>),
    Versions(String, Result<Vec<String>, String>),
    Env(EnvInfo),
    Progress(String),
    Done(Result<DoneInfo, String>),
}

#[derive(Debug)]
pub struct StartReq {
    pub target: UpgradeTarget,
    pub opts: RunOptions,
    pub save: bool,
}

/// Efeitos que o ciclo principal tem de executar.
#[derive(Debug)]
pub enum Cmd {
    Start(StartReq),
    LoadVersions(String),
    RefreshLatest,
    SaveNow,
}

pub struct Picker {
    pub filter: String,
    pub cursor: usize,
}

pub struct App {
    pub project_name: String,
    pub project_dir: PathBuf,
    pub cfg: Config,
    pub deps: Vec<Dependency>,
    pub pkg: usize,
    pkg_touched: bool,
    pub choices: Vec<VersionChoice>,
    pub choice: usize,
    pub choices_loading: bool,
    /// testes · tipos · build · relatório
    pub opts: [bool; 4],
    pub focus: Focus,
    pub screen: Screen,
    pub run: RunState,
    pub show_details: bool,
    pub scroll: u16,
    pub picker: Option<Picker>,
    pub latest_loading: bool,
    pub env: Option<EnvInfo>,
    pub log: Vec<(String, String)>,
    pub status: String,
    pub should_quit: bool,
    pub tick: usize,
    quit_armed: bool,
    pub reports: Vec<SavedReport>,
    pub reports_cursor: usize,
    pub envs: Vec<PathBuf>,
    pub envs_cursor: usize,
}

fn now() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

fn default_choices(dep: &Dependency) -> Vec<VersionChoice> {
    vec![VersionChoice {
        label: "latest".into(),
        version: dep.latest.clone(),
        is_latest: true,
    }]
}

impl App {
    pub fn new(project_name: String, project_dir: PathBuf, deps: Vec<Dependency>, cfg: Config) -> Self {
        let choices = deps.first().map(default_choices).unwrap_or_default();
        Self {
            project_name,
            project_dir,
            cfg,
            deps,
            pkg: 0,
            pkg_touched: false,
            choices,
            choice: 0,
            choices_loading: true,
            opts: [true; 4],
            focus: Focus::Start,
            screen: Screen::Upgrade,
            run: RunState::Idle,
            show_details: false,
            scroll: 0,
            picker: None,
            latest_loading: false,
            env: None,
            log: vec![],
            status: String::new(),
            should_quit: false,
            tick: 0,
            quit_armed: false,
            reports: vec![],
            reports_cursor: 0,
            envs: vec![],
            envs_cursor: 0,
        }
    }

    pub fn dep(&self) -> Option<&Dependency> {
        self.deps.get(self.pkg)
    }
    pub fn choice(&self) -> Option<&VersionChoice> {
        self.choices.get(self.choice)
    }
    pub fn running(&self) -> bool {
        matches!(self.run, RunState::Running { .. })
    }
    pub fn report(&self) -> Option<&Report> {
        match &self.run {
            RunState::Done { report, .. } => Some(report),
            _ => None,
        }
    }

    pub fn log_push(&mut self, msg: impl Into<String>) {
        self.log.push((now(), msg.into()));
        if self.log.len() > 200 {
            self.log.remove(0);
        }
    }

    // ------------------------------------------------------------------ mensagens
    pub fn on_msg(&mut self, m: Msg) -> Option<Cmd> {
        match m {
            Msg::Latest(Ok(map)) => {
                for d in &mut self.deps {
                    d.latest = map.get(&d.name).cloned();
                }
                self.latest_loading = false;
                let n = self.deps.iter().filter(|d| d.latest.is_some()).count();
                self.status = format!("{n} pacote(s) com versão nova");
                self.log_push(format!("{n} pacote(s) com versão nova"));
                // Se o utilizador ainda não escolheu nada, sugere o primeiro desatualizado.
                if !self.pkg_touched && !self.running() {
                    if let Some(i) = self.deps.iter().position(|d| d.latest.is_some()) {
                        if i != self.pkg {
                            return self.select_pkg(i, false);
                        }
                    }
                }
                if self.choices_loading {
                    if let Some(d) = self.deps.get(self.pkg) {
                        self.choices = default_choices(d);
                    }
                }
                None
            }
            Msg::Latest(Err(e)) => {
                self.latest_loading = false;
                self.status = format!("não foi possível consultar versões: {}", e.lines().next().unwrap_or(""));
                self.log_push(self.status.clone());
                None
            }
            Msg::Versions(name, res) => {
                if self.dep().map(|d| d.name.as_str()) != Some(name.as_str()) {
                    return None;
                }
                self.choices_loading = false;
                match res {
                    Ok(list) => {
                        let dep = self.deps[self.pkg].clone();
                        let found = major_choices(&list, clean_spec(&dep.current), 4);
                        if !found.is_empty() {
                            self.choices = found
                                .into_iter()
                                .map(|m| {
                                    let is_latest = dep.latest.as_deref() == Some(m.version.as_str())
                                        || (dep.latest.is_none() && m.is_latest);
                                    VersionChoice {
                                        label: format!("{}.x{}", m.major, if is_latest { " (latest)" } else { "" }),
                                        version: Some(m.version),
                                        is_latest,
                                    }
                                })
                                .collect();
                            self.choice = 0;
                        }
                    }
                    Err(e) => self.status = format!("versões de {name}: {}", e.lines().next().unwrap_or("")),
                }
                None
            }
            Msg::Env(e) => {
                self.env = Some(e);
                None
            }
            Msg::Progress(name) => {
                if let RunState::Running { steps, .. } = &mut self.run {
                    if let Some(last) = steps.last_mut() {
                        if last.duration.is_none() {
                            last.duration = Some(last.started.elapsed());
                        }
                    }
                    steps.push(LiveStep { name: name.clone(), started: Instant::now(), duration: None });
                }
                self.log_push(name);
                None
            }
            Msg::Done(Ok(info)) => {
                let (txt, _) = verdict_text(info.report.verdict);
                self.log_push(format!("Upgrade concluído: {txt}"));
                if let Some(Ok(s)) = &info.saved {
                    self.log_push(format!("Relatório salvo em {}", s.json));
                }
                self.status = format!("upgrade concluído: {txt}");
                self.run = RunState::Done { report: info.report, saved: info.saved };
                self.scroll = 0;
                None
            }
            Msg::Done(Err(e)) => {
                self.log_push(format!("Erro: {}", e.lines().next().unwrap_or("")));
                self.status = "o upgrade falhou (erro da ferramenta)".into();
                self.run = RunState::Failed(e);
                None
            }
        }
    }

    // ------------------------------------------------------------------ teclado
    pub fn on_key(&mut self, k: KeyEvent) -> Option<Cmd> {
        if self.picker.is_some() {
            return self.picker_key(k);
        }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let is_quit = (ctrl && k.code == KeyCode::Char('c')) || k.code == KeyCode::Char('q');
        if is_quit {
            if self.running() && !self.quit_armed {
                self.quit_armed = true;
                self.status = "teste em curso: prima q outra vez para sair (pode ficar lixo em /tmp)".into();
            } else {
                self.should_quit = true;
            }
            return None;
        }
        self.quit_armed = false;

        if ctrl && k.code == KeyCode::Char('k') {
            self.open_picker();
            return None;
        }
        match k.code {
            KeyCode::Esc => {
                self.back();
                return None;
            }
            KeyCode::Tab => return self.goto(self.screen.step(1)),
            KeyCode::BackTab => return self.goto(self.screen.step(-1)),
            KeyCode::Char(c @ '1'..='6') => return self.goto(Screen::ALL[c as usize - '1' as usize]),
            KeyCode::Char('?') => return self.goto(Screen::Docs),
            KeyCode::PageDown => {
                self.scroll = self.scroll.saturating_add(10);
                return None;
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(10);
                return None;
            }
            _ => {}
        }
        match self.screen {
            Screen::Upgrade => self.upgrade_key(k),
            Screen::Reports => {
                self.list_key(k, true);
                None
            }
            Screen::Envs => self.envs_key(k),
            _ => None,
        }
    }

    pub fn goto(&mut self, s: Screen) -> Option<Cmd> {
        self.screen = s;
        self.scroll = 0;
        match s {
            Screen::Reports => {
                self.reports = export::list_reports(&self.project_dir);
                self.reports_cursor = 0;
            }
            Screen::Envs => {
                self.reload_envs();
                self.envs_cursor = 0;
            }
            _ => {}
        }
        None
    }

    fn back(&mut self) {
        if self.show_details {
            self.show_details = false;
            self.scroll = 0;
        } else if self.screen != Screen::Upgrade {
            self.goto(Screen::Upgrade);
        } else {
            self.status = "q para sair".into();
        }
    }

    fn upgrade_key(&mut self, k: KeyEvent) -> Option<Cmd> {
        use Focus::*;
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.focus = match self.focus {
                    Pkg | Ver => Pkg,
                    Opt(0) | Opt(1) => Ver,
                    Opt(2) => Opt(0),
                    Opt(_) => Opt(1),
                    Start => Opt(2),
                };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.focus = match self.focus {
                    Pkg => Ver,
                    Ver => Opt(0),
                    Opt(0) => Opt(2),
                    Opt(1) => Opt(3),
                    Opt(_) | Start => Start,
                };
            }
            KeyCode::Left | KeyCode::Char('h') => match self.focus {
                Opt(1) => self.focus = Opt(0),
                Opt(3) => self.focus = Opt(2),
                Ver => self.cycle(-1),
                _ => {}
            },
            KeyCode::Right | KeyCode::Char('l') => match self.focus {
                Opt(0) => self.focus = Opt(1),
                Opt(2) => self.focus = Opt(3),
                Ver => self.cycle(1),
                _ => {}
            },
            KeyCode::Char(' ') => {
                if let Opt(i) = self.focus {
                    self.opts[i] = !self.opts[i];
                }
            }
            KeyCode::Enter => match self.focus {
                Pkg => self.open_picker(),
                Ver => self.cycle(1),
                Opt(i) => self.opts[i] = !self.opts[i],
                Start => return self.start(),
            },
            KeyCode::Char('t') => return self.start(),
            KeyCode::Char('d') => {
                if self.report().is_some() {
                    self.show_details = !self.show_details;
                    self.scroll = 0;
                } else {
                    self.status = "ainda não há resultado para detalhar".into();
                }
            }
            KeyCode::Char('s') => {
                return if self.report().is_some() {
                    Some(Cmd::SaveNow)
                } else {
                    self.status = "nada para guardar: execute um upgrade primeiro".into();
                    None
                };
            }
            KeyCode::Char('r')
                if !self.latest_loading => {
                    self.latest_loading = true;
                    self.status = "a consultar o registo...".into();
                    return Some(Cmd::RefreshLatest);
                }
            _ => {}
        }
        None
    }

    fn cycle(&mut self, delta: isize) {
        let n = self.choices.len() as isize;
        if n > 0 {
            self.choice = (self.choice as isize + delta).rem_euclid(n) as usize;
        }
    }

    fn start(&mut self) -> Option<Cmd> {
        if self.running() {
            self.status = "já há um upgrade em execução".into();
            return None;
        }
        let dep = self.dep()?.clone();
        if self.cfg.ignore.contains(&dep.name) {
            self.status = format!("'{}' está em `ignore` no depsandbox.toml", dep.name);
            return None;
        }
        let version = self.choice().and_then(|c| c.version.clone());
        let target = UpgradeTarget { package: dep.name, version };
        let opts = RunOptions { tests: self.opts[0], typecheck: self.opts[1], build: self.opts[2] };
        self.run = RunState::Running { started: Instant::now(), steps: vec![] };
        self.show_details = false;
        self.scroll = 0;
        self.log_push(format!("Iniciando upgrade {}", target.spec()));
        self.status = format!("a testar {}...", target.spec());
        Some(Cmd::Start(StartReq { target, opts, save: self.opts[3] }))
    }

    // ------------------------------------------------------------------ seletor de pacote
    fn open_picker(&mut self) {
        if self.running() {
            self.status = "não é possível trocar de pacote durante um upgrade".into();
            return;
        }
        self.screen = Screen::Upgrade;
        self.picker = Some(Picker { filter: String::new(), cursor: self.pkg });
    }

    pub fn picker_items(&self) -> Vec<usize> {
        let f = self.picker.as_ref().map(|p| p.filter.to_lowercase()).unwrap_or_default();
        self.deps
            .iter()
            .enumerate()
            .filter(|(_, d)| d.name.to_lowercase().contains(&f))
            .map(|(i, _)| i)
            .collect()
    }

    fn picker_key(&mut self, k: KeyEvent) -> Option<Cmd> {
        let items = self.picker_items();
        let p = self.picker.as_mut()?;
        match k.code {
            KeyCode::Esc => self.picker = None,
            KeyCode::Up => p.cursor = p.cursor.saturating_sub(1),
            KeyCode::Down => p.cursor = (p.cursor + 1).min(items.len().saturating_sub(1)),
            KeyCode::Backspace => {
                p.filter.pop();
                p.cursor = 0;
            }
            KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => {
                p.filter.push(c);
                p.cursor = 0;
            }
            KeyCode::Enter => {
                let cur = p.cursor;
                self.picker = None;
                if let Some(&i) = items.get(cur) {
                    return self.select_pkg(i, true);
                }
            }
            _ => {}
        }
        None
    }

    fn select_pkg(&mut self, i: usize, touched: bool) -> Option<Cmd> {
        self.pkg = i;
        self.pkg_touched |= touched;
        self.choices = default_choices(&self.deps[i]);
        self.choice = 0;
        self.choices_loading = true;
        self.run = RunState::Idle;
        self.show_details = false;
        self.scroll = 0;
        self.focus = Focus::Start;
        Some(Cmd::LoadVersions(self.deps[i].name.clone()))
    }

    // ------------------------------------------------------------------ listas
    fn list_key(&mut self, k: KeyEvent, reports: bool) {
        let (cur, len) = if reports {
            (&mut self.reports_cursor, self.reports.len())
        } else {
            (&mut self.envs_cursor, self.envs.len())
        };
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => *cur = cur.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => *cur = (*cur + 1).min(len.saturating_sub(1)),
            _ => return,
        }
        self.scroll = 0;
    }

    fn envs_key(&mut self, k: KeyEvent) -> Option<Cmd> {
        match k.code {
            KeyCode::Char('d') | KeyCode::Delete => {
                if let Some(p) = self.envs.get(self.envs_cursor).cloned() {
                    match std::fs::remove_dir_all(&p) {
                        Ok(()) => self.status = format!("removido {}", p.display()),
                        Err(e) => self.status = format!("não foi possível remover: {e}"),
                    }
                    self.reload_envs();
                    self.envs_cursor = self.envs_cursor.min(self.envs.len().saturating_sub(1));
                }
            }
            KeyCode::Char('r') => self.reload_envs(),
            _ => self.list_key(k, false),
        }
        None
    }

    fn reload_envs(&mut self) {
        let mut v: Vec<PathBuf> = std::fs::read_dir(std::env::temp_dir())
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter(|e| e.file_name().to_string_lossy().starts_with("depsandbox-") && e.path().is_dir())
                    .map(|e| e.path())
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        self.envs = v;
    }
}

pub fn verdict_text(v: Verdict) -> (&'static str, bool) {
    match v {
        Verdict::NoIssuesDetected => ("nenhum problema detectado", true),
        Verdict::BreakingChangesDetected => ("breaking changes detectadas", false),
        Verdict::BaselineBroken => ("baseline já falhava (comparação pouco fiável)", false),
        Verdict::InstallFailed => ("o upgrade não instalou", false),
    }
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
    fn code(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }
    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }
    fn dep(name: &str, cur: &str) -> Dependency {
        Dependency { name: name.into(), current: cur.into(), latest: None, dev: false }
    }
    fn app() -> App {
        let cfg = Config { ignore: vec!["zzz".into()], ..Default::default() };
        App::new("demo".into(), PathBuf::from("."), vec![dep("a", "^1.0.0"), dep("react", "19.1.0"), dep("zzz", "1.0.0")], cfg)
    }

    #[test]
    fn latest_auto_selects_first_outdated_and_loads_versions() {
        let mut a = app();
        let cmd = a.on_msg(Msg::Latest(Ok(HashMap::from([("react".into(), "19.2.0".into())]))));
        assert_eq!(a.pkg, 1);
        assert!(matches!(cmd, Some(Cmd::LoadVersions(ref n)) if n == "react"));

        let all: Vec<String> = ["18.3.1", "19.1.0", "19.2.0", "20.0.0-rc.1"].iter().map(|s| s.to_string()).collect();
        a.on_msg(Msg::Versions("react".into(), Ok(all)));
        assert_eq!(a.choices.len(), 1);
        assert_eq!(a.choices[0].label, "19.x (latest)");
        assert_eq!(a.choices[0].version.as_deref(), Some("19.2.0"));
        assert!(!a.choices_loading);
    }

    #[test]
    fn versions_for_other_package_are_ignored() {
        let mut a = app();
        a.on_msg(Msg::Versions("zzz".into(), Ok(vec!["9.0.0".into()])));
        assert!(a.choices_loading, "só aplica ao pacote selecionado");
    }

    #[test]
    fn start_builds_target_and_options() {
        let mut a = app();
        a.on_msg(Msg::Latest(Ok(HashMap::from([("a".into(), "2.0.0".into())]))));
        a.on_key(code(KeyCode::Up)); // Start -> Opt(2)
        a.on_key(code(KeyCode::Char(' '))); // desliga build
        assert!(!a.opts[2]);
        a.on_key(code(KeyCode::Down)); // volta a Start
        let Some(Cmd::Start(req)) = a.on_key(code(KeyCode::Enter)) else { panic!("esperava Start") };
        assert_eq!(req.target.spec(), "a@2.0.0");
        assert!(req.opts.tests && req.opts.typecheck && !req.opts.build);
        assert!(req.save);
        assert!(a.running());
        assert!(a.on_key(code(KeyCode::Enter)).is_none(), "sem segunda execução em simultâneo");
    }

    #[test]
    fn progress_builds_steps_with_durations() {
        let mut a = app();
        a.on_key(code(KeyCode::Enter));
        a.on_msg(Msg::Progress("Criando ambiente isolado".into()));
        a.on_msg(Msg::Progress("Instalando".into()));
        let RunState::Running { steps, .. } = &a.run else { panic!() };
        assert_eq!(steps.len(), 2);
        assert!(steps[0].duration.is_some() && steps[1].duration.is_none());
        assert!(a.log.len() >= 3);
    }

    #[test]
    fn ignored_package_is_refused() {
        let mut a = app();
        a.pkg = 2;
        assert!(a.on_key(code(KeyCode::Enter)).is_none());
        assert!(a.status.contains("ignore"));
        assert!(!a.running());
    }

    #[test]
    fn picker_filters_and_selects() {
        let mut a = app();
        a.on_key(ctrl('k'));
        assert!(a.picker.is_some());
        a.on_key(key('r'));
        a.on_key(key('e'));
        assert_eq!(a.picker_items(), vec![1]);
        let cmd = a.on_key(code(KeyCode::Enter));
        assert!(matches!(cmd, Some(Cmd::LoadVersions(ref n)) if n == "react"));
        assert_eq!(a.pkg, 1);
        assert!(a.picker.is_none());
    }

    #[test]
    fn navigation_between_screens_and_back() {
        let mut a = app();
        a.on_key(code(KeyCode::Tab));
        assert_eq!(a.screen, Screen::Reports);
        a.on_key(key('6'));
        assert_eq!(a.screen, Screen::About);
        a.on_key(code(KeyCode::Esc));
        assert_eq!(a.screen, Screen::Upgrade);
    }

    #[test]
    fn quit_needs_confirmation_while_running() {
        let mut a = app();
        a.on_key(code(KeyCode::Enter));
        a.on_key(key('q'));
        assert!(!a.should_quit);
        a.on_key(key('q'));
        assert!(a.should_quit);
    }
}
