use crate::session::Session;
use eframe::egui::{self, Rect, Sense};
use std::{collections::BTreeMap, path::PathBuf};
include!("dock_groups.rs");

#[derive(Clone, Copy, Debug)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}
enum Node {
    Leaf(u64),
    Split {
        vertical: bool,
        ratio: f32,
        a: Box<Node>,
        b: Box<Node>,
    },
}
impl Node {
    fn replace(&mut self, target: u64, id: u64) {
        match self {
            Self::Leaf(n) if *n == target => *n = id,
            Self::Split { a, b, .. } => {
                a.replace(target, id);
                b.replace(target, id);
            }
            _ => {}
        }
    }
    fn balanced(mut nodes: Vec<Node>, vertical: bool) -> Node {
        if nodes.len() == 1 {
            return nodes.remove(0);
        }
        let half = nodes.len() / 2;
        let total = nodes.len();
        let right = nodes.split_off(half);
        Node::Split {
            vertical,
            ratio: half as f32 / total as f32,
            a: Box::new(Self::balanced(nodes, vertical)),
            b: Box::new(Self::balanced(right, vertical)),
        }
    }
    fn grid(ids: &[u64], columns: usize) -> Node {
        Self::balanced(
            ids.chunks(columns)
                .map(|row| Self::balanced(row.iter().map(|id| Self::Leaf(*id)).collect(), false))
                .collect(),
            true,
        )
    }
    fn insert(&mut self, target: u64, id: u64, side: Side) -> bool {
        match self {
            Self::Leaf(n) if *n == target => {
                let old = Self::Leaf(*n);
                let new = Self::Leaf(id);
                let (a, b) = if matches!(side, Side::Left | Side::Top) {
                    (new, old)
                } else {
                    (old, new)
                };
                *self = Self::Split {
                    vertical: matches!(side, Side::Top | Side::Bottom),
                    ratio: 0.5,
                    a: Box::new(a),
                    b: Box::new(b),
                };
                true
            }
            Self::Split { a, b, .. } => a.insert(target, id, side) || b.insert(target, id, side),
            _ => false,
        }
    }
    fn remove(self, id: u64) -> Option<Self> {
        match self {
            Self::Leaf(n) => {
                if n == id {
                    None
                } else {
                    Some(Self::Leaf(n))
                }
            }
            Self::Split {
                vertical,
                ratio,
                a,
                b,
            } => match (a.remove(id), b.remove(id)) {
                (Some(a), Some(b)) => Some(Self::Split {
                    vertical,
                    ratio,
                    a: Box::new(a),
                    b: Box::new(b),
                }),
                (a, b) => a.or(b),
            },
        }
    }
    fn first(&self) -> u64 {
        match self {
            Self::Leaf(n) => *n,
            Self::Split { a, .. } => a.first(),
        }
    }
    fn leaves(&self, ids: &mut Vec<u64>) {
        match self {
            Self::Leaf(id) => ids.push(*id),
            Self::Split { a, b, .. } => {
                a.leaves(ids);
                b.leaves(ids);
            }
        }
    }
}
#[derive(Clone)]
pub struct Profile {
    pub shell: String,
    pub cwd: PathBuf,
    pub args: Vec<String>,
    pub connection: bool,
}
impl Profile {
    pub fn start(&self) -> anyhow::Result<Session> {
        if self.connection {
            Session::connect(&self.args, &self.cwd)
        } else {
            Session::start(&self.shell, &self.args, &self.cwd)
        }
    }
}
struct Pane {
    profile: Profile,
    session: Session,
}
enum Action {
    Error(String),
    Close(u64),
    Restart(u64),
    Split(u64, Side),
    Float(u64),
    Move(u64, u64, Side),
}
#[derive(Default)]
pub struct Dock {
    root: Option<Node>,
    panes: BTreeMap<u64, Pane>,
    primary: Option<Profile>,
    active: u64,
    next: u64,
    floating: Vec<u64>,
    pub message: String,
    pending: Option<(u64, bool)>,
    primary_closed: bool,
    pub global_action: Option<String>,
    tabs: bool,
    split_view: bool,
    split_root: Option<Node>,
    groups: crate::terminal_groups::Groups,
    tab_target: Option<u64>,
}
impl Dock {
    pub fn overview(&self, primary: Option<&Session>) -> Vec<(String, bool)> {
        primary
            .filter(|_| !self.primary_closed)
            .zip(self.primary.as_ref())
            .map(|(session, profile)| (session_title(profile), !session.exited))
            .into_iter()
            .chain(
                self.panes
                    .values()
                    .map(|pane| (session_title(&pane.profile), !pane.session.exited)),
            )
            .collect()
    }

    pub fn has_sessions(&self) -> bool {
        self.root.is_some() || !self.floating.is_empty()
    }
    pub fn has_graphical_view(&self, primary: Option<&Session>) -> bool {
        if !self.tabs && !self.split_view {
            return false;
        }
        primary.is_some_and(|s| s.ui_view.is_some())
            || self.panes.values().any(|p| p.session.ui_view.is_some())
    }
    pub fn launch(&mut self, profile: Profile) -> anyhow::Result<()> {
        let session = profile.start()?;
        self.next += 1;
        let id = self.next;
        self.panes.insert(id, Pane { profile, session });
        let target = self.root.as_ref().map(Node::first).unwrap_or(id);
        self.attach(id, target, Side::Right);
        if let Some(group) = self.tab_target.take() {
            self.groups.add(group, id);
        }
        self.sync_groups();
        Ok(())
    }
    pub fn split_active(&mut self) {
        self.apply(Action::Split(self.active, Side::Right));
    }
    pub fn verify(_exe: &std::path::Path, cwd: &std::path::Path) -> anyhow::Result<()> {
        #[cfg(windows)]
        let profile = Profile {
            shell: "powershell.exe".into(),
            cwd: cwd.to_path_buf(),
            args: [
                "-NoLogo", "-NoProfile", "-Command",
                "while ($null -ne ($line = [Console]::ReadLine())) { [Console]::WriteLine('ACK:' + $line) }",
            ].map(str::to_string).into(),
            connection: false,
        };
        #[cfg(not(windows))]
        let profile = Profile {
            shell: _exe.to_string_lossy().into_owned(),
            cwd: cwd.to_path_buf(),
            args: vec!["--probe-session".into()],
            connection: false,
        };
        let mut primary = Session::start(&profile.shell, &profile.args, cwd)?;
        let mut dock = Self::default();
        dock.primary(profile);
        dock.split_active();
        anyhow::ensure!(
            dock.panes.len() == 1,
            "Split did not create a real session: {}",
            dock.message
        );
        dock.apply(Action::Float(1));
        anyhow::ensure!(dock.floating == [1], "Session did not detach");
        dock.apply(Action::Move(1, 0, Side::Bottom));
        anyhow::ensure!(
            dock.floating.is_empty() && dock.panes.len() == 1,
            "Session did not redock"
        );
        let ctx = egui::Context::default();
        for tabs in [true, false, true] {
            dock.tabs = tabs;
            dock.split_view = false;
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 600.0),
                )),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default()
                    .show(ctx, |ui| dock.show(ui, &mut primary, false, 15.0));
            });
        }
        anyhow::ensure!(
            dock.panes.len() == 1 && dock.root.is_some(),
            "Switching view changed the session layout"
        );
        dock.sync_groups();
        let group = dock
            .groups
            .items
            .iter()
            .find(|g| g.tabs.contains(&1))
            .unwrap()
            .id;
        dock.add_group_tab(group, &primary);
        let new_tab = dock.active;
        anyhow::ensure!(
            dock.groups.items.len() == 2
                && dock
                    .groups
                    .items
                    .iter()
                    .find(|g| g.id == group)
                    .unwrap()
                    .tabs
                    == [1, new_tab],
            "New tab escaped its grid group"
        );
        for active in [1, new_tab, 1] {
            dock.active = active;
            dock.groups.select(active);
            dock.tabs = true;
            dock.split_view = false;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1100.0, 700.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .show(ctx, |ui| dock.show(ui, &mut primary, false, 15.0));
                },
            );
            let columns = dock.panes[&active].session.parser.screen().size().1;
            anyhow::ensure!(
                columns >= 100,
                "Opened group did not use full terminal width: {columns} columns"
            );
        }
        println!("PASS: tabs remain inside their grid square; opening and switching uses full-width PTYs");
        for count in [2, 4, 6, 8, 2] {
            dock.split_preset(count)?;
            let mut ids = vec![];
            dock.split_root.as_ref().unwrap().leaves(&mut ids);
            anyhow::ensure!(ids.len() == count, "Wrong split count");
            let unique: std::collections::BTreeSet<_> = ids.iter().collect();
            anyhow::ensure!(unique.len() == count, "A split reused the same terminal");
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200.0, 800.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .show(ctx, |ui| dock.show(ui, &mut primary, false, 15.0));
                },
            );
            for id in ids {
                dock.session_mut(&mut primary, id)
                    .unwrap()
                    .send(b"SPLIT_ALIVE\r")?;
            }
        }
        let until = std::time::Instant::now() + std::time::Duration::from_secs(8);
        loop {
            primary.poll();
            dock.poll(&ctx);
            let alive = primary
                .parser
                .screen()
                .contents()
                .contains("ACK:SPLIT_ALIVE")
                && dock
                    .panes
                    .iter()
                    .filter(|(id, _)| **id != new_tab)
                    .all(|(_, p)| {
                        p.session
                            .parser
                            .screen()
                            .contents()
                            .contains("ACK:SPLIT_ALIVE")
                    });
            if alive {
                break;
            }
            anyhow::ensure!(
                std::time::Instant::now() < until,
                "A split terminal did not accept input"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        dock.active = 1;
        dock.groups.select(1);
        println!("PASS: 2/4/6/8-way layouts each run independent terminals; reducing splits preserves sessions");
        dock.send(&mut primary, b"DOCK_INPUT_OK\r")?;
        let until = std::time::Instant::now() + std::time::Duration::from_secs(8);
        while std::time::Instant::now() < until {
            dock.poll(&egui::Context::default());
            if dock.panes[&1]
                .session
                .parser
                .screen()
                .contents()
                .contains("ACK:DOCK_INPUT_OK")
            {
                println!("PASS: switching Sections/Tabs preserves the live session and layout");
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        anyhow::bail!("Moved session did not acknowledge input")
    }
    pub fn primary(&mut self, profile: Profile) {
        self.primary = Some(profile);
        if self.primary_closed {
            self.primary_closed = false;
            if let Some(root) = &mut self.root {
                root.insert(root.first(), 0, Side::Right);
            }
        }
        if self.root.is_none() {
            self.root = Some(Node::Leaf(0));
        }
    }
    pub fn poll(&mut self, ctx: &egui::Context) {
        for pane in self.panes.values_mut() {
            pane.session.set_context(ctx);
            pane.session.poll();
            pane.profile.cwd = pane.session.cwd.clone();
        }
    }
    pub fn refresh_primary(&mut self, session: &Session) {
        if let Some(profile) = &mut self.primary {
            profile.cwd = session.cwd.clone();
        }
    }
    pub fn profile(&self) -> Option<&Profile> {
        if self.active == 0 {
            self.primary.as_ref()
        } else {
            self.panes.get(&self.active).map(|p| &p.profile)
        }
    }
    pub fn send(&mut self, primary: &mut Session, bytes: &[u8]) -> anyhow::Result<()> {
        if self.active == 0 {
            primary.send(bytes)
        } else {
            self.panes
                .get_mut(&self.active)
                .ok_or_else(|| anyhow::anyhow!("No active pane"))?
                .session
                .send(bytes)
        }
    }
    pub fn copy(&self, primary: &Session) -> String {
        if self.active == 0 {
            primary.copy_text()
        } else {
            self.panes
                .get(&self.active)
                .map(|p| p.session.copy_text())
                .unwrap_or_default()
        }
    }
    fn attach(&mut self, id: u64, target: u64, side: Side) {
        self.floating.retain(|n| *n != id);
        self.root = self.root.take().and_then(|n| n.remove(id));
        if let Some(root) = &mut self.root {
            if !root.insert(target, id, side) {
                let first = root.first();
                root.insert(first, id, side);
            }
        } else {
            self.root = Some(Node::Leaf(id));
        }
        self.active = id;
    }
    fn apply(&mut self, action: Action) {
        match action {
            Action::Error(error) => self.message = error,
            Action::Close(id) => self.pending = Some((id, false)),
            Action::Restart(id) => self.pending = Some((id, true)),
            Action::Move(id, target, side) if id != target => {
                self.split_root = None;
                self.attach(id, target, side);
            }
            Action::Float(id) => {
                self.split_root = self.split_root.take().and_then(|n| n.remove(id));
                self.root = self.root.take().and_then(|n| n.remove(id));
                if self.active == id {
                    if let Some(root) = &self.root {
                        self.active = root.first();
                    }
                }
                if !self.floating.contains(&id) {
                    self.floating.push(id);
                }
            }
            Action::Split(target, side) => {
                self.tabs = false;
                self.split_view = true;
                self.split_root = None;
                let profile = if target == 0 {
                    self.primary.clone()
                } else {
                    self.panes.get(&target).map(|p| p.profile.clone())
                };
                if let Some(profile) = profile {
                    match profile.start() {
                        Ok(session) => {
                            self.next += 1;
                            let id = self.next;
                            self.panes.insert(id, Pane { profile, session });
                            self.attach(id, target, side);
                        }
                        Err(e) => self.message = format!("Split: {e}"),
                    }
                }
            }
            _ => {}
        }
    }
    fn session_mut<'a>(&'a mut self, primary: &'a mut Session, id: u64) -> Option<&'a mut Session> {
        if id == 0 {
            Some(primary)
        } else {
            self.panes.get_mut(&id).map(|p| &mut p.session)
        }
    }
    pub fn has_live(&self, primary: &Session) -> bool {
        (!self.primary_closed && !primary.exited) || self.panes.values().any(|p| !p.session.exited)
    }
    pub fn focus(&mut self, primary: &mut Session) {
        let id = self.active;
        if let Some(s) = self.session_mut(primary, id) {
            s.interaction.focus_requested = true;
        }
    }
    pub fn handle(
        &mut self,
        ctx: &egui::Context,
        primary: &mut Session,
        bindings: &crate::keybindings::Bindings,
    ) {
        for action in ["new_window", "keybindings", "dashboard"] {
            if bindings.take(ctx, action) {
                self.global_action = Some(action.into());
                ctx.request_repaint_of(egui::ViewportId::ROOT);
            }
        }
        let toggle = bindings.take(ctx, "toggle_terminal_view");
        let grid = bindings.take(ctx, "terminal_grid");
        let splits = bindings.take(ctx, "terminal_splits");
        if grid || splits || toggle {
            let show_splits = splits || (toggle && !self.split_view);
            self.tabs = false;
            self.split_view = show_splits;
            if show_splits {
                self.focus(primary);
            }
            ctx.request_repaint();
        }
        if bindings.take(ctx, "split_right") {
            self.apply(Action::Split(self.active, Side::Right));
        }
        if bindings.take(ctx, "split_below") {
            self.apply(Action::Split(self.active, Side::Bottom));
        }
        if bindings.take(ctx, "detach_pane") {
            self.apply(Action::Float(self.active));
        }
        if bindings.take(ctx, "dock_pane") {
            let target = self.root.as_ref().map(Node::first).unwrap_or(self.active);
            self.attach(self.active, target, Side::Right);
        }
        if bindings.take(ctx, "close_pane") {
            self.pending = Some((self.active, false));
        }
        if bindings.take(ctx, "restart_pane") {
            self.pending = Some((self.active, true));
        }
        if bindings.take(ctx, "terminal_focus") {
            self.focus(primary);
        }
        let next = bindings.take(ctx, "next_pane");
        let prev = bindings.take(ctx, "previous_pane");
        if next || prev {
            let mut ids = vec![];
            if !self.primary_closed {
                ids.push(0);
            }
            ids.extend(self.panes.keys().copied());
            if self.tabs {
                if let Some(group) = self
                    .visible_groups()
                    .iter()
                    .find(|g| g.tabs.contains(&self.active))
                {
                    ids = group.tabs.clone();
                }
            }
            if !ids.is_empty() {
                let i = ids.iter().position(|n| *n == self.active).unwrap_or(0);
                self.active = ids[(i + if next { 1 } else { ids.len() - 1 }) % ids.len()];
                self.groups.select(self.active);
                self.focus(primary);
            }
        }
        if bindings.take(ctx, "copy_terminal") {
            ctx.copy_text(self.copy(primary));
        }
    }
    pub fn confirm(&mut self, ctx: &egui::Context, primary: &mut Session) {
        let Some((id, restart)) = self.pending else {
            return;
        };
        let live = self.session_mut(primary, id).is_some_and(|s| !s.exited);
        let mut accept = !live;
        let mut cancel = false;
        let sibling = self
            .groups
            .items
            .iter()
            .find(|g| g.tabs.contains(&id))
            .and_then(|g| g.tabs.iter().copied().find(|tab| *tab != id));
        if live {
            egui::Window::new(if restart {
                "Restart session?"
            } else {
                "Close session?"
            })
            .collapsible(false)
            .resizable(false)
            .frame(
                egui::Frame::NONE
                    .inner_margin(24.0)
                    .fill(ctx.style().visuals.window_fill),
            )
            .show(ctx, |ui| {
                ui.label("This will end the shell and its running processes.");
                ui.horizontal(|ui| {
                    accept = crate::widgets::button(ui, "End session").clicked();
                    cancel = crate::widgets::button(ui, "Keep running").clicked();
                });
            });
        }
        if cancel {
            self.pending = None;
        }
        if accept {
            self.pending = None;
            if restart {
                let profile = if id == 0 {
                    self.primary.clone()
                } else {
                    self.panes.get(&id).map(|p| p.profile.clone())
                };
                if let Some(profile) = profile {
                    if let Some(s) = self.session_mut(primary, id) {
                        s.terminate();
                    }
                    match profile.start() {
                        Ok(s) => {
                            if id == 0 {
                                *primary = s;
                                self.primary_closed = false;
                            } else {
                                self.panes.insert(
                                    id,
                                    Pane {
                                        profile,
                                        session: s,
                                    },
                                );
                            }
                        }
                        Err(e) => self.message = format!("Restart: {e}"),
                    }
                }
            } else {
                if id == 0 {
                    primary.terminate();
                    self.primary_closed = true;
                } else {
                    self.panes.remove(&id);
                }
                self.root = self.root.take().and_then(|r| r.remove(id));
                if let Some(root) = &mut self.split_root {
                    if let Some(sibling) = sibling {
                        root.replace(id, sibling);
                    }
                }
                if sibling.is_none() {
                    self.split_root = self.split_root.take().and_then(|n| n.remove(id));
                }
                self.floating.retain(|n| *n != id);
                if self.active == id {
                    self.active = sibling
                        .or_else(|| self.root.as_ref().map(Node::first))
                        .or_else(|| self.floating.first().copied())
                        .unwrap_or(0);
                }
                self.sync_groups();
                self.groups.select(self.active);
            }
        }
    }
    pub fn set_syntax(&mut self, enabled: bool) {
        for pane in self.panes.values_mut() {
            pane.session.interaction.line.syntax = enabled;
        }
    }
    pub fn show(&mut self, ui: &mut egui::Ui, primary: &mut Session, light: bool, size: f32) {
        self.sync_groups();
        ui.horizontal(|ui| {
            if crate::widgets::text_tab(ui, "Grid", !self.tabs && !self.split_view).clicked() {
                self.tabs = false;
                self.split_view = false;
            }
            if self.tabs && crate::widgets::button(ui, "Back to grid").clicked() {
                self.tabs = false;
            }
            if crate::widgets::text_tab(ui, "Splits", self.split_view).clicked() {
                self.tabs = false;
                self.split_view = true;
            }
            if self.split_view {
                for count in [2, 4, 6, 8] {
                    if crate::widgets::button(ui, format!("{count} way")).clicked() {
                        if let Err(error) = self.split_preset(count) {
                            self.message = format!("Split layout: {error}");
                        }
                    }
                }
            }
        });
        ui.add_space(8.0);
        let mut actions = vec![];
        if self.split_view {
            let rect = ui.available_rect_before_wrap();
            if let Some(mut root) = self.split_root.take() {
                self.draw_split_groups(ui, rect, &mut root, primary, light, size, &mut actions);
                self.split_root = Some(root);
            } else if let Some(root) = &mut self.root {
                draw(
                    ui,
                    rect,
                    root,
                    &mut self.panes,
                    primary,
                    &mut self.active,
                    &mut actions,
                    light,
                    size,
                );
            }
            ui.advance_cursor_after_rect(rect);
        } else if self.tabs {
            self.group_expanded(ui, primary, light, size, &mut actions);
        } else {
            self.group_grid(ui, primary, light, &mut actions);
        }
        for action in actions {
            self.apply(action);
        }
    }
    pub fn windows(
        &mut self,
        ctx: &egui::Context,
        primary: &mut Session,
        light: bool,
        size: f32,
        bindings: &crate::keybindings::Bindings,
    ) {
        let mut returned = vec![];
        for id in self.floating.clone() {
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of(("session", id)),
                egui::ViewportBuilder::default()
                    .with_icon(crate::branding::icon())
                    .with_title(crate::APP_NAME)
                    .with_decorations(false)
                    .with_inner_size([900.0, 620.0]),
                |ctx, _| {
                    self.handle(ctx, primary, bindings);
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE.inner_margin(16.0).fill(if light {
                            egui::Color32::from_rgb(250, 250, 250)
                        } else {
                            egui::Color32::from_rgb(17, 17, 17)
                        }))
                        .show(ctx, |ui| {
                            ui.horizontal(|ui| {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if crate::chrome::controls(ui) {
                                            self.pending = Some((id, false));
                                        }
                                        if crate::widgets::button(ui, "Dock").clicked() {
                                            returned.push(id);
                                        }
                                        crate::chrome::title(ui, crate::APP_NAME);
                                    },
                                );
                            });
                            ui.add_space(12.0);
                            ui.push_id(id, |ui| {
                                let session = if id == 0 {
                                    Some(&mut *primary)
                                } else {
                                    self.panes.get_mut(&id).map(|p| &mut p.session)
                                };
                                if let Some(session) = session {
                                    session.interaction.active = self.active == id;
                                    if let Err(e) = crate::terminal::draw(ui, session, light, size)
                                    {
                                        self.message = e.to_string();
                                    }
                                }
                            });
                            if ui.rect_contains_pointer(ui.max_rect())
                                && ui.input(|i| i.pointer.any_pressed())
                            {
                                self.active = id;
                            }
                        });
                    if ctx.input(|i| i.viewport().close_requested()) {
                        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                        self.pending = Some((id, false));
                    }
                    crate::chrome::resize(ctx);
                    self.confirm(ctx, primary);
                },
            );
        }
        returned.sort_unstable();
        returned.dedup();
        for id in returned {
            let target = self.root.as_ref().map(Node::first).unwrap_or(id);
            self.attach(id, target, Side::Right);
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn draw(
    ui: &mut egui::Ui,
    rect: Rect,
    node: &mut Node,
    panes: &mut BTreeMap<u64, Pane>,
    primary: &mut Session,
    active: &mut u64,
    actions: &mut Vec<Action>,
    light: bool,
    size: f32,
) {
    match node {
        Node::Split {
            vertical,
            ratio,
            a,
            b,
        } => {
            let span = if *vertical {
                rect.height()
            } else {
                rect.width()
            };
            let offset = span * *ratio;
            let (ra, gap, rb) = if *vertical {
                (
                    Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.min.y + offset - 5.0)),
                    Rect::from_min_max(
                        egui::pos2(rect.min.x, rect.min.y + offset - 5.0),
                        egui::pos2(rect.max.x, rect.min.y + offset + 5.0),
                    ),
                    Rect::from_min_max(egui::pos2(rect.min.x, rect.min.y + offset + 5.0), rect.max),
                )
            } else {
                (
                    Rect::from_min_max(rect.min, egui::pos2(rect.min.x + offset - 5.0, rect.max.y)),
                    Rect::from_min_max(
                        egui::pos2(rect.min.x + offset - 5.0, rect.min.y),
                        egui::pos2(rect.min.x + offset + 5.0, rect.max.y),
                    ),
                    Rect::from_min_max(egui::pos2(rect.min.x + offset + 5.0, rect.min.y), rect.max),
                )
            };
            let response = ui
                .interact(
                    gap,
                    ui.id().with(("divider", a.first(), b.first())),
                    Sense::drag(),
                )
                .on_hover_cursor(if *vertical {
                    egui::CursorIcon::ResizeVertical
                } else {
                    egui::CursorIcon::ResizeHorizontal
                });
            if response.dragged() {
                let delta = ui.input(|i| {
                    if *vertical {
                        i.pointer.delta().y
                    } else {
                        i.pointer.delta().x
                    }
                });
                *ratio = (*ratio + delta / span.max(1.0)).clamp(0.15, 0.85);
            }
            let alpha = if response.hovered() || response.dragged() {
                0x44
            } else {
                0x22
            };
            let tone = if light {
                egui::Color32::from_black_alpha(alpha)
            } else {
                egui::Color32::from_white_alpha(alpha)
            };
            let points = if *vertical {
                [
                    egui::pos2(gap.left() + 8.0, gap.center().y),
                    egui::pos2(gap.right() - 8.0, gap.center().y),
                ]
            } else {
                [
                    egui::pos2(gap.center().x, gap.top() + 8.0),
                    egui::pos2(gap.center().x, gap.bottom() - 8.0),
                ]
            };
            ui.painter()
                .line_segment(points, egui::Stroke::new(1.0_f32, tone));
            draw(ui, ra, a, panes, primary, active, actions, light, size);
            draw(ui, rb, b, panes, primary, active, actions, light, size);
        }
        Node::Leaf(id) => {
            let rect = rect.shrink(8.0_f32.min(rect.width().min(rect.height()).max(0.0) / 4.0));
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
                ui.set_width(rect.width());
                ui.set_height(rect.height());
                ui.set_clip_rect(rect);
                ui.push_id(*id, |ui| {
                    let shell = if *id == 0 {
                        &primary.shell
                    } else {
                        &panes[id].profile.shell
                    };
                    let short_shell = std::path::Path::new(shell)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy();
                    let shell_name = if short_shell.eq_ignore_ascii_case("pwsh.exe")
                        || short_shell.eq_ignore_ascii_case("pwsh")
                    {
                        "PowerShell"
                    } else if short_shell.eq_ignore_ascii_case("powershell.exe")
                        || short_shell.eq_ignore_ascii_case("powershell")
                    {
                        "Windows PowerShell"
                    } else if short_shell.eq_ignore_ascii_case("cmd.exe")
                        || short_shell.eq_ignore_ascii_case("cmd")
                    {
                        "Command Prompt"
                    } else {
                        short_shell.as_ref()
                    };
                    let cwd = if *id == 0 {
                        &primary.cwd
                    } else {
                        &panes[id].session.cwd
                    };
                    let name = format!(
                        "{} / {}",
                        shell_name,
                        cwd.file_name().unwrap_or_default().to_string_lossy()
                    );
                    ui.horizontal(|ui| {
                        let name = if *active == *id {
                            format!("{name} · Active")
                        } else {
                            name.clone()
                        };
                        let (header, response) = ui.allocate_exact_size(
                            egui::vec2((ui.available_width() - 44.0).max(40.0), 28.0),
                            Sense::click_and_drag(),
                        );
                        ui.painter().with_clip_rect(header).text(
                            header.left_center(),
                            egui::Align2::LEFT_CENTER,
                            &name,
                            egui::TextStyle::Body.resolve(ui.style()),
                            if *active == *id {
                                ui.visuals().hyperlink_color
                            } else {
                                ui.visuals().text_color()
                            },
                        );
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name)
                        });
                        let response =
                            response.on_hover_text("Drag to dock or detach this session");
                        if response.clicked() {
                            *active = *id;
                        }
                        if response.drag_started() {
                            egui::DragAndDrop::set_payload(ui.ctx(), *id);
                        }
                        if response.drag_stopped()
                            && !ui.ctx().input(|i| {
                                i.pointer
                                    .hover_pos()
                                    .is_some_and(|p| ui.ctx().screen_rect().contains(p))
                            })
                        {
                            actions.push(Action::Float(*id));
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let menu = ui.menu_button(" ", |ui| {
                                for (label, action) in [
                                    ("Split right", Action::Split(*id, Side::Right)),
                                    ("Split below", Action::Split(*id, Side::Bottom)),
                                    ("Detach window", Action::Float(*id)),
                                    ("Restart session", Action::Restart(*id)),
                                    ("Close session", Action::Close(*id)),
                                ] {
                                    if crate::widgets::button(ui, label).clicked() {
                                        actions.push(action);
                                        ui.close_menu();
                                    }
                                }
                            });
                            crate::icons::paint(
                                ui.painter(),
                                egui::Rect::from_center_size(
                                    menu.response.rect.center(),
                                    egui::Vec2::splat(18.0),
                                ),
                                crate::icons::Icon::More,
                                ui.visuals().weak_text_color(),
                            );
                            menu.response.on_hover_text("Session actions");
                        });
                    });
                    ui.add_space(12.0);
                    let session = if *id == 0 {
                        Some(&mut *primary)
                    } else {
                        panes.get_mut(id).map(|p| &mut p.session)
                    };
                    if let Some(session) = session {
                        session.interaction.active = *active == *id;
                        if session.exited {
                            ui.label("Shell exited / Restart or Close");
                        }
                        if let Err(e) = crate::terminal::draw(ui, session, light, size) {
                            actions.push(Action::Error(format!("Terminal: {e}")));
                        }
                    }
                    if rect.contains(ui.input(|i| i.pointer.hover_pos().unwrap_or_default()))
                        && ui.input(|i| i.pointer.any_pressed())
                    {
                        *active = *id;
                    }
                    if let Some(source) =
                        egui::DragAndDrop::payload::<u64>(ui.ctx()).filter(|n| **n != *id)
                    {
                        if let Some(p) = ui
                            .input(|i| i.pointer.hover_pos())
                            .filter(|p| rect.contains(*p))
                        {
                            let x = (p.x - rect.min.x) / rect.width();
                            let y = (p.y - rect.min.y) / rect.height();
                            let side = if x < 0.25 {
                                Side::Left
                            } else if x > 0.75 {
                                Side::Right
                            } else if y < 0.5 {
                                Side::Top
                            } else {
                                Side::Bottom
                            };
                            let mut hint = rect;
                            match side {
                                Side::Left => hint.max.x = rect.center().x,
                                Side::Right => hint.min.x = rect.center().x,
                                Side::Top => hint.max.y = rect.center().y,
                                Side::Bottom => hint.min.y = rect.center().y,
                            };
                            ui.painter().rect_filled(
                                hint,
                                0.0,
                                if light {
                                    egui::Color32::from_black_alpha(35)
                                } else {
                                    egui::Color32::from_white_alpha(25)
                                },
                            );
                            if ui.input(|i| i.pointer.any_released()) {
                                actions.push(Action::Move(*source, *id, side));
                            }
                        }
                    }
                });
            });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rearrangement_preserves_sessions() {
        let mut root = Node::Leaf(0);
        assert!(root.insert(0, 1, Side::Right));
        assert!(root.insert(1, 2, Side::Bottom));
        let mut root = root.remove(1).unwrap();
        assert!(root.insert(0, 1, Side::Left));
        let root = root.remove(0).unwrap().remove(1).unwrap();
        assert_eq!(root.first(), 2);
        assert!(root.remove(2).is_none());
    }
}

fn session_title(profile: &Profile) -> String {
    if profile.connection {
        let protocol = profile
            .args
            .first()
            .map(String::as_str)
            .unwrap_or("Connection");
        let endpoint = profile
            .args
            .windows(2)
            .find(|pair| pair[0] == "--profile" || pair[0] == "--host")
            .map(|pair| pair[1].as_str())
            .or_else(|| profile.args.get(1).map(String::as_str));
        return endpoint.map_or_else(|| protocol.into(), |name| format!("{protocol} · {name}"));
    }
    let shell = std::path::Path::new(&profile.shell)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let folder = profile
        .cwd
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    if folder.is_empty() {
        shell.into_owned()
    } else {
        format!("{shell} · {folder}")
    }
}
