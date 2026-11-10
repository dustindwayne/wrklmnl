use crate::{bridge::Bridge, history::History, session::Session, Options};
use eframe::egui::{self, Color32, RichText};
#[cfg(feature = "capture")]
use std::time::Duration;
use std::{collections::BTreeMap, path::PathBuf, process::Command};
use terminal_ui_kit::{Element, View};
use terminal_ui_language::Document;

pub struct App {
    options: Options,
    history: History,
    scratchpad: crate::scratchpad::Scratchpad,
    history_ok: bool,
    folder: String,
    shell: String,
    active_folder: String,
    active_shell: String,
    session: Option<Session>,
    shell_catalog: crate::shell_catalog::Catalog,
    workspace: crate::workspace_ui::Workspace,
    show_shell_picker: bool,
    file_picker: crate::file_picker::Picker,
    bar_picker: Option<crate::bar_background::Bar>,
    shell_picker_file: bool,
    settings_tab: u8,
    custom_shells: crate::custom_shells::Shells,
    top_background: crate::bar_background::ImageSlot,
    side_background: crate::bar_background::ImageSlot,
    bridge: Option<Bridge>,
    view: Option<View>,
    values: BTreeMap<String, String>,
    status: String,
    page: u8,
    focus_terminal: bool,
    document: Option<Document>,
    recent_query: String,
    appearance: crate::appearance::Appearance,
    #[cfg(feature = "capture")]
    capture: Option<String>,
    #[cfg(feature = "capture")]
    capture_started: std::time::Instant,
    #[cfg(feature = "capture")]
    capture_requested: bool,
    editor: crate::editor::Editor,
    dock: crate::dock::Dock,
    bindings: crate::keybindings::Bindings,
    connections: crate::connections::panel::Panel,
    launcher: crate::launcher::Workspace,
    appearance_edit: crate::appearance::Appearance,
    quit_pending: bool,
    quit_approved: bool,
}
impl App {
    fn capturing(&self) -> bool {
        #[cfg(feature = "capture")]
        {
            self.capture.is_some()
        }
        #[cfg(not(feature = "capture"))]
        {
            false
        }
    }
    pub fn new(cc: &eframe::CreationContext<'_>, mut options: Options) -> Self {
        let (history, history_ok, mut status) = match History::load(&options.state) {
            Ok(h) => (h, true, "Ready".into()),
            Err(e) => (
                History::default(),
                false,
                format!("Recent locations could not load: {e}. Existing file preserved."),
            ),
        };
        let mut appearance = match crate::appearance::Appearance::load(
            &options.state.with_file_name("appearance.json"),
        ) {
            Ok(a) => a,
            Err(e) => {
                status = format!("Appearance could not load: {e}. Existing file preserved.");
                Default::default()
            }
        };
        if std::env::args().any(|a| a == "--light") {
            appearance.light = true;
        }
        if std::env::args().any(|a| a == "--accent") {
            appearance.accent = options.accent;
        }
        options.light = appearance.light;
        options.accent = appearance.accent;
        crate::theme::apply(&cc.egui_ctx, options.light, options.accent);
        if let Some(font) = &options.font {
            appearance.terminal_font = font.clone();
        }
        if let Some(size) = options.font_size {
            appearance.terminal_size = size;
        }
        if let Err(e) = appearance.apply(&cc.egui_ctx) {
            status = format!("Font settings: {e}");
        }
        let launcher = crate::launcher::Workspace::new(&options.state, &options.cwd);
        let mut app = Self {
            folder: crate::history::display_path(if options.session {
                &options.cwd
            } else {
                launcher.root()
            }),
            shell: options.shell.clone(),
            active_folder: String::new(),
            active_shell: String::new(),
            session: None,
            shell_catalog: Default::default(),
            workspace: Default::default(),
            show_shell_picker: false,
            file_picker: Default::default(),
            bar_picker: None,
            shell_picker_file: false,
            settings_tab: 1,
            custom_shells: crate::custom_shells::Shells::load(
                &options.state.with_file_name("custom-shells.json"),
            ),
            top_background: Default::default(),
            side_background: Default::default(),
            bridge: None,
            view: None,
            values: BTreeMap::new(),
            status,
            history,
            scratchpad: crate::scratchpad::Scratchpad::load(
                options.state.with_file_name("scratchpad.json"),
            ),
            history_ok,
            page: 0,
            focus_terminal: true,
            document: None,
            recent_query: String::new(),
            appearance: appearance.clone(),
            #[cfg(feature = "capture")]
            capture: std::env::var("TERMINAL_CAPTURE_TO").ok(),
            #[cfg(feature = "capture")]
            capture_started: std::time::Instant::now(),
            #[cfg(feature = "capture")]
            capture_requested: false,
            editor: Default::default(),
            dock: Default::default(),
            bindings: crate::keybindings::Bindings::load(
                &options.state.with_file_name("keybindings.json"),
            ),
            connections: Default::default(),
            launcher,
            appearance_edit: appearance.clone(),
            quit_pending: false,
            quit_approved: false,
            options,
        };
        if app.options.session {
            app.open_session();
            #[cfg(feature = "capture")]
            if std::env::var_os("TERMINAL_CAPTURE_SPLIT").is_some() {
                app.dock.split_active();
            }
            #[cfg(feature = "capture")]
            if std::env::var_os("TERMINAL_CAPTURE_GROUPS").is_some() {
                if let Some(primary) = &app.session {
                    app.dock.prepare_group_capture(
                        primary,
                        std::env::var_os("TERMINAL_CAPTURE_GROUP_OPEN").is_some(),
                    );
                }
            }
        }
        if app.options.demo
            || app.options.document.is_some()
            || app.options.embedded_source.is_some()
        {
            let source = if let Some(source) = &app.options.embedded_source {
                Ok(source.clone())
            } else if let Some(path) = &app.options.document {
                std::fs::read_to_string(path).map_err(|e| e.to_string())
            } else {
                Ok(include_str!("../../../examples/ui-example.txt").into())
            };
            match source.and_then(|s| Document::parse(&s)).and_then(|d| {
                let v = d.render(&app.binding_data())?;
                Ok((d, v))
            }) {
                Ok((document, view)) => {
                    app.document = Some(document);
                    app.set_view(view);
                }
                Err(e) => app.status = format!("UI source: {e}"),
            }
            app.page = 2;
        }
        if let Some(program) = &app.options.ui_program {
            match Bridge::start(program, &app.options.ui_args) {
                Ok(b) => {
                    app.bridge = Some(b);
                    app.page = 2;
                }
                Err(e) => app.status = format!("UI app: {e}"),
            }
        }
        if let Some(path) = app.options.open_file.clone() {
            app.open_editor(path);
        }
        if app.options.terminal {
            app.page = 1;
        }
        if app.options.keybindings {
            app.page = 6;
        }
        if app.options.appearance {
            app.page = 4;
        }
        #[cfg(feature = "capture")]
        if app.capture.is_some() {
            if let Ok(page) = std::env::var("DTERM_CAPTURE_PAGE") {
                if let Ok(page) = page.parse::<u8>() {
                    app.page = page;
                }
            }
        }
        #[cfg(feature = "capture")]
        if app.capture.is_some() && std::env::var_os("DTERM_CAPTURE_SAMPLE").is_some() {
            if let Ok(target) = std::env::var("DTERM_CAPTURE_FILE") {
                let path = app.launcher.root().join(target);
                app.launcher.capture_select(path, &cc.egui_ctx);
            }
            if let Ok(view) = std::env::var("DTERM_CAPTURE_CONNECTION") {
                app.connections.capture_view(&view, app.launcher.root());
            }
        }
        app
    }
    fn accent(&self) -> Color32 {
        crate::theme::accent(self.options.light, self.options.accent)
    }
    fn binding_data(&self) -> BTreeMap<String, String> {
        let mut data = self.options.data.clone();
        data.extend(self.values.clone());
        data.insert("status".into(), self.status.clone());
        data.insert(
            "recent".into(),
            self.history
                .locations
                .iter()
                .map(|l| crate::history::display_path(&l.path))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        data
    }
    fn muted(&self) -> Color32 {
        if self.options.light {
            Color32::from_rgb(98, 98, 98)
        } else {
            Color32::from_rgb(163, 163, 163)
        }
    }
    fn set_view(&mut self, view: View) {
        // Preserve typed values when a streamed document repeats unchanged defaults.
        let mut previous = BTreeMap::new();
        if let Some(old) = &self.view {
            collect_defaults(&old.elements, &mut previous);
        }
        let mut present = std::collections::BTreeSet::new();
        let mut pending: Vec<_> = view.elements.iter().collect();
        while let Some(e) = pending.pop() {
            match e {
                Element::Input { id, value, .. } => {
                    present.insert(id.clone());
                    if previous.get(id) != Some(value) || !self.values.contains_key(id) {
                        self.values.insert(id.clone(), value.clone());
                    }
                }
                Element::Stack { children, .. } => pending.extend(children.iter()),
                Element::Place { child, .. } => pending.push(child),
                _ => {}
            }
        }
        self.values.retain(|id, _| present.contains(id));
        self.view = Some(view);
    }
    fn open_session(&mut self) {
        let path = PathBuf::from(&self.folder);
        match path.canonicalize().and_then(|p| {
            if p.is_dir() {
                Ok(p)
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Choose a directory",
                ))
            }
        }) {
            Ok(path) => {
                let profile = crate::dock::Profile {
                    shell: self.shell.clone(),
                    cwd: path.clone(),
                    args: self.options.args.clone(),
                    connection: false,
                };
                let result = if self.session.is_some() {
                    self.dock.launch(profile)
                } else {
                    Session::start(&self.shell, &self.options.args, &path).map(|session| {
                        self.dock.primary(profile);
                        self.session = Some(session);
                    })
                };
                match result {
                    Ok(()) => {
                        self.page = 1;
                        self.folder = crate::history::display_path(&path);
                        self.active_folder = self.folder.clone();
                        self.active_shell = self.shell.clone();
                        self.focus_terminal = true;
                        self.show_shell_picker = false;
                        self.status = "Shell connected".into();
                        if self.history_ok {
                            self.history
                                .record_with_args(&path, &self.shell, &self.options.args);
                            if let Err(e) = self.history.save(&self.options.state) {
                                self.status =
                                    format!("Session opened; recent locations could not save: {e}");
                            }
                        }
                    }
                    Err(e) => self.status = format!("Could not start {}: {e}", self.shell),
                }
            }
            Err(e) => self.status = format!("Working folder: {e}"),
        }
    }
    fn new_window(&mut self) {
        let result = (|| -> anyhow::Result<()> {
            let (folder, shell) = if self.page == 1 && self.session.is_some() {
                (&self.active_folder, &self.active_shell)
            } else {
                (&self.folder, &self.shell)
            };
            anyhow::ensure!(
                PathBuf::from(folder).is_dir(),
                "Working folder does not exist"
            );
            let mut cmd = Command::new(std::env::current_exe()?);
            cmd.arg("--session")
                .arg(folder)
                .arg("--shell")
                .arg(shell)
                .arg("--state-file")
                .arg(&self.options.state);
            if self.options.light {
                cmd.arg("--light");
            }
            cmd.arg("--accent").arg(format!(
                "{:02x}{:02x}{:02x}",
                self.options.accent[0], self.options.accent[1], self.options.accent[2]
            ));
            if let Some(font) = &self.options.font {
                cmd.arg("--font").arg(font);
            }
            if let Some(size) = self.options.font_size {
                cmd.arg("--font-size").arg(size.to_string());
            }
            if !self.options.args.is_empty() {
                cmd.arg("--").args(&self.options.args);
            }
            cmd.spawn()?;
            Ok(())
        })();
        if let Err(e) = result {
            self.status = format!("New window: {e}");
        }
    }
    fn open_editor(&mut self, path: PathBuf) {
        let path = if path.is_absolute() {
            path
        } else {
            self.options.cwd.join(path)
        };
        match self.editor.request_open(path) {
            Ok(()) => {
                self.page = 5;
                self.status = if self.editor.dirty() {
                    "Editor has unsaved changes"
                } else {
                    "File opened in editor"
                }
                .into();
            }
            Err(e) => self.status = format!("Could not open file: {e}"),
        }
    }
    fn launch_choice(&mut self, choice: crate::shell_catalog::Choice) {
        self.shell = choice.executable.to_string_lossy().into_owned();
        self.options.args = choice.args;
        self.open_session();
    }
    fn workspace_values(&self) -> BTreeMap<String, String> {
        BTreeMap::from([("folder".into(), self.folder.clone())])
    }
    fn workspace_render(
        &mut self,
        ui: &mut egui::Ui,
        id: &str,
        source: &str,
        values: BTreeMap<String, String>,
        selected: Option<&str>,
    ) {
        let accent = self.accent();
        self.workspace.surface.light = self.options.light;
        self.workspace.surface.syntax = self.appearance.syntax;
        self.workspace.surface.ui_size = self.appearance.ui_size;
        self.workspace.surface.size = self.appearance.terminal_size;
        self.workspace.surface.accent_color = accent;
        let events = match self.workspace.render(ui, id, source, values, selected) {
            Ok(events) => events,
            Err(error) => {
                self.status = format!("Workspace UI: {error}");
                ui.label(&self.status);
                return;
            }
        };
        if let Some(folder) = self.workspace.surface.values.get("folder") {
            self.folder = folder.clone();
        }
        for event in events {
            let action = event.action.as_str();
            match action {
                "open_file" => self.file_picker.start(PathBuf::from(&self.folder)),
                "refresh_shells" => self.shell_catalog.refresh(),
                "manage_shells" => {
                    self.settings_tab = 2;
                    self.page = 4;
                }
                "back_sessions" => {
                    self.show_shell_picker = false;
                    self.dock.cancel_tab_target();
                }
                "new_window" => self.new_window(),
                _ => {
                    if let Some(index) = action
                        .strip_prefix("custom_profile_")
                        .and_then(|index| index.parse::<usize>().ok())
                    {
                        if let Some(profile) = self.custom_shells.profiles.get(index).cloned() {
                            match profile.program() {
                                Ok(program) => {
                                    if !profile.folder.is_empty() {
                                        self.folder = profile.folder;
                                    }
                                    self.shell = program.to_string_lossy().into_owned();
                                    self.options.args = profile.args;
                                    self.open_session();
                                }
                                Err(error) => {
                                    self.status =
                                        format!("Could not launch {}: {error:#}", profile.name)
                                }
                            }
                        }
                    } else if let Some(index) = action
                        .strip_prefix("shell_")
                        .and_then(|s| s.parse::<usize>().ok())
                    {
                        if let Some(choice) = self.shell_catalog.items().get(index).cloned() {
                            self.launch_choice(choice);
                        }
                    } else if let Some(page) = action
                        .strip_prefix("page_")
                        .and_then(|s| s.parse::<u8>().ok())
                    {
                        if matches!(page, 0 | 1 | 3 | 4 | 5 | 6 | 7 | 8)
                            && (page != 5 || self.editor.has_document())
                        {
                            self.page = page;
                            if page != 1 {
                                self.dock.cancel_tab_target();
                                self.show_shell_picker = false;
                            }
                            self.focus_terminal = page == 1;
                        }
                    }
                }
            }
        }
    }
    fn launcher_source(
        &mut self,
        ctx: &egui::Context,
        values: &mut BTreeMap<String, String>,
    ) -> String {
        self.shell_catalog.poll(ctx);
        values.insert(
            "shell_status".into(),
            if self.shell_catalog.loading() {
                "Detecting installed shells..."
            } else if self.shell_catalog.items().is_empty() {
                "No installed shells found. You can add a custom shell in Settings."
            } else {
                ""
            }
            .into(),
        );
        let custom = if self.custom_shells.profiles.is_empty() {
            "gap:0".into()
        } else {
            let tiles = self.custom_shells.profiles.iter().enumerate().map(|(index, profile)| {
                values.insert(format!("custom_name_{index}"), profile.name.clone());
                values.insert(format!("custom_program_{index}"), profile.executable.clone());
                format!(r#"tile[ico:terminal > "{{custom_name_{index}}}"@custom_profile_{index} > t:$custom_program_{index}]"#)
            }).collect::<Vec<_>>().join(" > ");
            format!(r#"col:12[hdr:"Custom shells" > grid:12[{tiles}]]"#)
        };
        crate::workspace_ui::LAUNCHER
            .replace(
                "#SHELLS",
                &crate::workspace_ui::shells(self.shell_catalog.items(), values),
            )
            .replace("content[] #CUSTOM", &custom)
    }
    fn workspace_navigation(&mut self, ui: &mut egui::Ui, compact: bool) {
        let mut source = crate::workspace_ui::NAVIGATION.replace(
            "content[] #EDITOR",
            if self.editor.has_document() {
                "item[ico:code > Editor@page_5]"
            } else {
                "gap:0"
            },
        );
        if compact {
            source = source
                .replace(r#"t:"Workspace""#, "gap:0")
                .replace(r#"t:"Build""#, "gap:0");
        }
        let selected = format!("page_{}", self.page);
        self.workspace_render(
            ui,
            "navigation",
            &source,
            self.workspace_values(),
            Some(&selected),
        );
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            self.workspace_render(
                ui,
                "new_window",
                r#"item[ico:plus > "New window"@new_window]"#,
                self.workspace_values(),
                None,
            );
            if !compact {
                ui.label(RichText::new("Ctrl+Shift+N").small().color(self.muted()));
            }
        });
    }
    fn terminal_page(&mut self, ui: &mut egui::Ui) {
        let has_sessions = self.session.is_some() && self.dock.has_sessions();
        if !has_sessions || self.show_shell_picker {
            let mut values = self.workspace_values();
            let launcher = self.launcher_source(ui.ctx(), &mut values);
            let source = crate::workspace_ui::PICKER
                .replace("content[] #LAUNCHER", &launcher)
                .replace(
                    "content[] #BACK",
                    if has_sessions {
                        "\"Back to sessions\"@back_sessions"
                    } else {
                        "gap:0"
                    },
                );
            self.workspace_render(ui, "terminal_picker", &source, values, None);
            return;
        }
        ui.horizontal(|ui| {
            ui.heading("Terminal");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::widgets::button(ui, "+ New session").clicked() {
                    self.dock.cancel_tab_target();
                    self.show_shell_picker = true;
                }
                if crate::widgets::button(ui, "Open file").clicked() {
                    self.file_picker.start(PathBuf::from(&self.folder));
                }
            });
        });
        ui.add_space(12.0);
        if let Some(session) = &mut self.session {
            session.interaction.line.syntax = self.appearance.syntax;
            self.dock.set_syntax(self.appearance.syntax);
            if self.focus_terminal {
                self.dock.focus(session);
                self.focus_terminal = false;
            }
            self.dock.show(
                ui,
                session,
                self.options.light,
                self.appearance.terminal_size,
            );
        }
    }
    fn home(&mut self, ui: &mut egui::Ui, viewport_height: f32) {
        let top = ui.next_widget_position().y;
        let radius = egui::CornerRadius::same(4);
        let widgets = &mut ui.visuals_mut().widgets;
        for widget in [
            &mut widgets.inactive,
            &mut widgets.hovered,
            &mut widgets.active,
            &mut widgets.open,
        ] {
            widget.corner_radius = radius;
        }
        let width = ui.available_width().min(1120.0);
        ui.set_max_width(width);
        ui.horizontal(|ui| {
            ui.heading("Dashboard");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if crate::widgets::button(ui, "Recent locations").clicked() {
                    self.page = 8;
                }
            });
        });
        ui.separator();
        ui.add_space(8.0);

        let padding = 16;
        let panel = egui::Frame::NONE
            .inner_margin(padding)
            .corner_radius(radius);
        let notes_height =
            viewport_height - (ui.next_widget_position().y - top) - 2.0 * f32::from(padding) - 2.0;
        let divider = Color32::from_gray(if self.options.light { 219 } else { 52 });
        let notes = panel
            .fill(Color32::from_gray(if self.options.light {
                255
            } else {
                22
            }))
            .stroke(egui::Stroke::new(1.0_f32, divider));
        let activity = panel.fill(Color32::from_gray(if self.options.light {
            243
        } else {
            20
        }));
        let render_notes = |app: &mut Self, ui: &mut egui::Ui, width: f32| {
            notes
                .show(ui, |ui| {
                    ui.set_width(width - 2.0 * f32::from(padding) - 2.0);
                    app.scratchpad.show(ui, app.launcher.root(), notes_height)
                })
                .inner
        };
        let render_activity = |app: &mut Self, ui: &mut egui::Ui, width: f32| {
            activity.show(ui, |ui| {
                ui.set_width(width - 2.0 * f32::from(padding));
                app.dashboard_activity(ui);
            });
        };
        let target = if width >= 680.0 {
            ui.horizontal_top(|ui| {
                let gap = 20.0;
                ui.spacing_mut().item_spacing.x = gap;
                let notes_width = ((width - gap) * 0.62).min(width - gap - 260.0);
                let activity_width = width - gap - notes_width;
                let column = egui::Layout::top_down(egui::Align::Min);
                let target = ui
                    .allocate_ui_with_layout(egui::vec2(notes_width, 0.0), column, |ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        render_notes(self, ui, notes_width)
                    })
                    .inner;
                ui.allocate_ui_with_layout(egui::vec2(activity_width, 0.0), column, |ui| {
                    render_activity(self, ui, activity_width);
                });
                target
            })
            .inner
        } else {
            let target = render_notes(self, ui, width);
            ui.add_space(8.0);
            render_activity(self, ui, width);
            target
        };
        if let Some(target) = target {
            use crate::scratchpad::Target;
            match target {
                Target::Page(page) if matches!(page, 0 | 1 | 3 | 4 | 6 | 7 | 8) => {
                    self.page = page;
                    self.focus_terminal = page == 1;
                }
                Target::File(path) => match self.launcher.reveal(path, true, ui.ctx()) {
                    Ok(()) => self.page = 7,
                    Err(error) => self.status = format!("{error:#}"),
                },
                Target::Folder(path) => match self.launcher.reveal(path, false, ui.ctx()) {
                    Ok(()) => self.page = 7,
                    Err(error) => self.status = format!("{error:#}"),
                },
                _ => {}
            }
        }
    }

    fn dashboard_activity(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 7.0;
        if self.connections.overview(ui) {
            self.page = 3;
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);
        ui.heading("Terminals");
        let sessions = self.dock.overview(self.session.as_ref());
        if sessions.is_empty() {
            ui.weak("No terminals open");
        } else {
            let active = sessions.iter().filter(|(_, running)| *running).count();
            ui.weak(format!("{active} running · {} total", sessions.len()));
        }
        for (name, running) in sessions.iter().take(6) {
            ui.label(format!(
                "{name} · {}",
                if *running { "Running" } else { "Exited" }
            ));
        }
        if crate::widgets::button(
            ui,
            if sessions.is_empty() {
                "Choose a shell"
            } else {
                "Open Terminals"
            },
        )
        .clicked()
        {
            self.page = 1;
            self.show_shell_picker = sessions.is_empty();
            self.focus_terminal = !sessions.is_empty();
        }
    }

    fn recent_locations(&mut self, ui: &mut egui::Ui) {
        ui.heading("Recent locations");
        ui.weak("Reopen a folder with its last shell.");
        ui.add_space(12.0);
        let width = ui.available_width();
        ui.add(
            egui::TextEdit::singleline(&mut self.recent_query)
                .hint_text("Filter by folder or shell")
                .desired_width(width),
        );
        ui.add_space(6.0);
        let query = self.recent_query.trim().to_lowercase();
        let mut reopen = None;
        let mut remove = None;
        let mut shown = 0;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for location in &self.history.locations {
                    let path = crate::history::display_path(&location.path);
                    let shell = std::path::Path::new(&location.shell)
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy();
                    if !format!("{path} {shell}").to_lowercase().contains(&query) {
                        continue;
                    }
                    shown += 1;
                    let name = location
                        .path
                        .file_name()
                        .unwrap_or(location.path.as_os_str())
                        .to_string_lossy();
                    let available = location.path.is_dir();
                    ui.push_id((&location.path, &location.shell, &location.args), |ui| {
                        ui.horizontal(|ui| {
                            let row_width =
                                (ui.available_width() - 44.0 - ui.spacing().item_spacing.x)
                                    .max(1.0);
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(row_width, 54.0),
                                egui::Sense::click(),
                            );
                            if response.hovered() || response.has_focus() {
                                ui.painter()
                                    .rect_filled(rect, 0.0, ui.visuals().selection.bg_fill);
                            }
                            let icon = egui::Rect::from_center_size(
                                egui::pos2(rect.left() + 17.0, rect.center().y),
                                egui::Vec2::splat(20.0),
                            );
                            crate::icons::paint(
                                ui.painter(),
                                icon,
                                crate::icons::Icon::Folder,
                                ui.visuals().hyperlink_color,
                            );
                            let painter = ui.painter().with_clip_rect(rect);
                            let detail = if available {
                                format!("{path} · {shell}")
                            } else {
                                format!("Folder unavailable · {path}")
                            };
                            for (text, top, font, color) in [
                                (
                                    name.as_ref(),
                                    7.0,
                                    egui::TextStyle::Body,
                                    ui.visuals().text_color(),
                                ),
                                (
                                    &detail,
                                    30.0,
                                    egui::TextStyle::Small,
                                    ui.visuals().weak_text_color(),
                                ),
                            ] {
                                let mut job = egui::text::LayoutJob::simple(
                                    text.into(),
                                    font.resolve(ui.style()),
                                    color,
                                    (row_width - 45.0).max(1.0),
                                );
                                job.wrap.max_rows = 1;
                                painter.galley(
                                    egui::pos2(rect.left() + 36.0, rect.top() + top),
                                    ui.fonts(|fonts| fonts.layout_job(job)),
                                    color,
                                );
                            }
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    available,
                                    name.as_ref(),
                                )
                            });
                            let enter = response.has_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter));
                            if available && (response.on_hover_text(&detail).clicked() || enter) {
                                reopen = Some(location.clone());
                            }
                            crate::icons::menu(ui, "Location actions", |ui| {
                                if ui
                                    .add_enabled(available, egui::Button::new("Open terminal here"))
                                    .clicked()
                                {
                                    reopen = Some(location.clone());
                                    ui.close_menu();
                                }
                                if ui.button("Copy folder path").clicked() {
                                    ui.ctx().copy_text(path.clone());
                                    ui.close_menu();
                                }
                                if ui.button("Remove from history").clicked() {
                                    remove = Some(location.clone());
                                    ui.close_menu();
                                }
                            });
                        });
                        ui.separator();
                    });
                }
                if shown == 0 {
                    ui.weak(if self.history.locations.is_empty() {
                        "Folders you open will appear here."
                    } else {
                        "No matching locations."
                    });
                }
            });
        if let Some(location) = remove {
            self.history.remove(&location);
            if let Err(error) = self.history.save(&self.options.state) {
                self.status = format!("Recent locations could not save: {error}");
            }
        }
        if let Some(location) = reopen {
            self.folder = crate::history::display_path(&location.path);
            self.shell = location.shell;
            self.options.args = location.args;
            self.open_session();
        }
    }
    fn open_connection(&mut self, args: Vec<String>) -> anyhow::Result<()> {
        let profile = crate::dock::Profile {
            shell: "Connections".into(),
            cwd: self.launcher.root().to_path_buf(),
            args,
            connection: true,
        };
        if self.session.is_some() {
            self.dock.launch(profile)?;
        } else {
            let session = profile.start()?;
            self.dock.primary(profile);
            self.session = Some(session);
        }
        self.page = 1;
        self.focus_terminal = true;
        Ok(())
    }
    fn framework(&mut self, ui: &mut egui::Ui) {
        let Some(view) = self.view.clone() else {
            ui.heading("UI framework");
            ui.label("Connect an app using --ui-program, or open the sample below.");
            if crate::widgets::add(ui, egui::Button::new("Open framework demo").frame(false))
                .clicked()
            {
                match Document::parse(include_str!("../../../examples/ui-example.txt")) {
                    Ok(d) => {
                        if let Ok(v) = d.render(&BTreeMap::new()) {
                            self.document = Some(d);
                            self.set_view(v);
                        }
                    }
                    Err(e) => self.status = e,
                }
            }
            return;
        };
        ui.heading(&view.title);
        ui.add_space(16.0);
        self.render_elements(ui, &view, &view.elements);
    }
    fn render_elements(&mut self, ui: &mut egui::Ui, view: &View, elements: &[Element]) {
        let mut surface = crate::ui_runtime::Surface {
            values: std::mem::take(&mut self.values),
            design_mode: false,
            syntax: self.appearance.syntax,
            light: self.options.light,
            size: self.appearance.terminal_size,
            ui_size: self.appearance.ui_size,
            accent_color: self.accent(),
            installed_icons: std::mem::take(&mut self.workspace.surface.installed_icons),
            ..Default::default()
        };
        surface.render_elements(ui, view, elements);
        self.values = surface.values;
        self.workspace.surface.installed_icons = surface.installed_icons;
        for event in surface.events {
            let id = event.action.as_str();
            if let Some(bridge) = &mut self.bridge {
                if let Err(e) = bridge.send(&event) {
                    self.status = format!("UI action: {e}");
                }
            } else if id == "new" && self.options.embedded_source.is_none() {
                self.new_window();
            } else if id == "home" && self.options.embedded_source.is_none() {
                self.page = 0;
            } else if id == "terminal" && self.options.embedded_source.is_none() {
                self.page = 1;
            } else if let Some(document) = &self.document {
                match document.render(&self.binding_data()) {
                    Ok(view) => {
                        self.set_view(view);
                        self.status = format!("Action '{id}' / bindings updated");
                    }
                    Err(e) => self.status = e,
                }
            }
        }
    }
}
impl Drop for App {
    fn drop(&mut self) {
        self.scratchpad.flush();
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if self.options.light {
            Color32::from_rgb(250, 250, 250)
        } else {
            Color32::from_rgb(17, 17, 17)
        }
        .to_normalized_gamma_f32()
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.scratchpad.tick(ctx);
        if self.page != 7 {
            if let Some(path) = ctx.input(|i| {
                i.raw
                    .dropped_files
                    .iter()
                    .find_map(|file| file.path.clone())
            }) {
                self.open_editor(path);
            }
        }
        #[cfg(feature = "capture")]
        if let Some(path) = &self.capture {
            let capture_delay = if std::env::var_os("TERMINAL_CAPTURE_GROUPS").is_some() {
                6
            } else {
                2
            };
            if !self.capture_requested
                && self.capture_started.elapsed() > Duration::from_secs(capture_delay)
            {
                self.capture_requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            for event in ctx.input(|i| i.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                    if let Err(e) = image::save_buffer(
                        path,
                        &bytes,
                        image.width() as u32,
                        image.height() as u32,
                        image::ColorType::Rgba8,
                    ) {
                        eprintln!("Preview capture: {e}");
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        if self.bindings.take(ctx, "new_window") {
            self.new_window();
        }
        if self.bindings.take(ctx, "keybindings") {
            self.page = 6;
        }
        if self.bindings.take(ctx, "dashboard") {
            self.page = 0;
        }
        if self.bindings.take(ctx, "terminal_focus") {
            self.page = 1;
            self.focus_terminal = true;
        }
        if self.page == 1 {
            if let Some(s) = &mut self.session {
                self.dock.handle(ctx, s, &self.bindings);
            }
        }
        self.dock.poll(ctx);
        if let Some(profile) = self.dock.profile() {
            self.active_shell = profile.shell.clone();
            self.active_folder = crate::history::display_path(&profile.cwd);
        }
        if let Some(session) = &mut self.session {
            let was = session.exited;
            session.set_context(ctx);
            session.poll();
            self.dock.refresh_primary(session);
            if !was && session.exited {
                self.status = "Shell exited / output remains available to copy".into();
            }
        }
        if let Some(bridge) = &mut self.bridge {
            bridge.set_context(ctx);
            if !bridge.connected() {
                self.status =
                    "UI application disconnected / reconnect before sending actions".into();
            }
        }
        let messages = if let Some(bridge) = &self.bridge {
            bridge.messages.try_iter().take(16).collect::<Vec<_>>()
        } else {
            vec![]
        };
        for message in messages {
            match message {
                Ok(view) => {
                    self.set_view(view);
                }
                Err(e) => self.status = format!("UI protocol: {e}"),
            }
        }
        let top = &self.appearance_edit.top_background;
        let side = &self.appearance_edit.side_background;
        for error in [
            self.top_background.update(ctx, top),
            self.side_background.update(ctx, side),
        ]
        .into_iter()
        .flatten()
        {
            self.status = error;
        }
        egui::TopBottomPanel::top("navigation")
            .show_separator_line(false)
            .frame(
                egui::Frame::NONE
                    .fill(if self.options.light {
                        Color32::from_gray(245)
                    } else {
                        Color32::from_gray(20)
                    })
                    .inner_margin(egui::Margin::symmetric(20, 10)),
            )
            .show(ctx, |ui| {
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(crate::APP_NAME.into()));
                let background = ui.painter().add(egui::Shape::Noop);
                let header = ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if crate::chrome::controls(ui) {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        crate::chrome::title(ui, crate::APP_NAME);
                    });
                });
                self.top_background.paint(
                    ui.painter(),
                    background,
                    header.response.rect.expand2(egui::vec2(20.0, 10.0)),
                    top,
                );
            });
        egui::TopBottomPanel::bottom("status")
            .show_separator_line(false)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(24, 12)))
            .show(ctx, |ui| {
                let error = [
                    "error",
                    "could not",
                    "failed",
                    "invalid",
                    "busy",
                    "disconnected",
                    "terminal:",
                ]
                .iter()
                .any(|word| self.status.to_lowercase().contains(word));
                ui.label(RichText::new(&self.status).color(if error {
                    ui.visuals().error_fg_color
                } else {
                    self.muted()
                }));
            });
        if self.options.embedded_source.is_some() {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.inner_margin(24))
                .show(ctx, |ui| {
                    egui::ScrollArea::both().show(ui, |ui| {
                        if let Some(view) = self.view.clone() {
                            self.render_elements(ui, &view, &view.elements);
                        }
                    });
                });
            return;
        }
        let compact = ctx.screen_rect().width() < 940.0;
        let graphical_app = self.page == 2
            || (self.page == 1 && self.dock.has_graphical_view(self.session.as_ref()));
        if !graphical_app {
            egui::SidePanel::left("workspace_navigation")
                .resizable(false)
                .exact_width(if compact { 68.0 } else { 188.0 })
                .show_separator_line(false)
                .frame(
                    egui::Frame::NONE
                        .inner_margin(egui::Margin::symmetric(12, 20))
                        .fill(if self.options.light {
                            Color32::from_rgb(243, 243, 243)
                        } else {
                            Color32::from_rgb(22, 22, 22)
                        }),
                )
                .show(ctx, |ui| {
                    let background = ui.painter().add(egui::Shape::Noop);
                    let bounds = ui.max_rect().expand2(egui::vec2(12.0, 20.0));
                    self.side_background.paint(
                        ui.painter(),
                        background,
                        bounds,
                        &self.appearance_edit.side_background,
                    );
                    self.workspace_navigation(ui, compact);
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(28, 24)))
            .show(ctx, |ui| {
                if self.file_picker.open {
                    if let Some(path) = self.file_picker.show(ui) {
                        if self.shell_picker_file {
                            self.custom_shells.set_executable(&path);
                        } else {
                            match self.bar_picker.take() {
                                Some(crate::bar_background::Bar::Top) => {
                                    self.appearance_edit.top_background.path = Some(path)
                                }
                                Some(crate::bar_background::Bar::Side) => {
                                    self.appearance_edit.side_background.path = Some(path)
                                }
                                None => self.open_editor(path),
                            }
                        }
                    }
                    if !self.file_picker.open {
                        self.shell_picker_file = false;
                        self.bar_picker = None;
                    }
                    return;
                }
                match self.page {
                    0 => {
                        let viewport_height = ui.available_height();
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| self.home(ui, viewport_height));
                    }
                    1 => {
                        self.terminal_page(ui);
                    }
                    2 => {
                        egui::ScrollArea::both().show(ui, |ui| self.framework(ui));
                    }
                    3 => {
                        if let Some(args) = self.connections.show(ui, self.launcher.root()) {
                            if let Err(error) = self.open_connection(args) {
                                self.status = format!("Connection could not start: {error}");
                            }
                        }
                    }
                    7 => match self.launcher.show(ui) {
                        Some(crate::launcher::Action::Terminal(folder)) => {
                            self.folder = crate::history::display_path(&folder);
                            self.open_session();
                        }
                        Some(crate::launcher::Action::Editor(path)) => self.open_editor(path),
                        Some(crate::launcher::Action::Open(path)) => {
                            if let Err(error) = crate::launcher::open(&path) {
                                self.status = format!("Could not open item: {error}");
                            }
                        }
                        None => {}
                    },
                    8 => self.recent_locations(ui),
                    4 => {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.heading("Settings");
                            ui.horizontal_wrapped(|ui| {
                                for (tab, name) in
                                    [(0, "Workspace"), (1, "Appearance"), (2, "Custom shells")]
                                {
                                    ui.selectable_value(&mut self.settings_tab, tab, name);
                                }
                            });
                            ui.separator();
                            match self.settings_tab {
                                0 => {
                                    self.launcher.settings(ui);
                                    return;
                                }
                                2 => {
                                    let path =
                                        self.options.state.with_file_name("custom-shells.json");
                                    if self.custom_shells.show(ui, &path) {
                                        self.shell_picker_file = true;
                                        self.file_picker
                                            .start_executable(self.launcher.root().to_path_buf());
                                    }
                                    return;
                                }
                                _ => {}
                            }
                            let mut choose = None;
                            let save = self.appearance_edit.controls(ui, &mut choose);
                            if let Some(bar) = choose {
                                self.bar_picker = Some(bar);
                                self.file_picker
                                    .start_image(self.launcher.root().to_path_buf());
                            }
                            let accent = self.appearance_edit.accent;
                            if self.options.accent != accent {
                                self.options.accent = accent;
                                crate::theme::apply(ctx, self.options.light, accent);
                                ctx.request_repaint();
                            }
                            if save {
                                self.appearance = self.appearance_edit.clone();
                                self.options.light = self.appearance.light;
                                self.options.accent = self.appearance.accent;
                                crate::theme::apply(ctx, self.options.light, self.options.accent);
                                match self.appearance.commit(
                                    ctx,
                                    &self.options.state.with_file_name("appearance.json"),
                                ) {
                                    Ok(message) => self.status = message,
                                    Err(e) => {
                                        self.status = format!("Appearance could not save: {e}")
                                    }
                                }
                            }
                        });
                    }
                    5 => {
                        let editor_cwd = if self.active_folder.is_empty() {
                            self.options.cwd.clone()
                        } else {
                            PathBuf::from(&self.active_folder)
                        };
                        self.editor.show(
                            ui,
                            self.options.light,
                            self.appearance.terminal_size,
                            self.appearance.syntax,
                            &editor_cwd,
                        );
                        if !self.editor.has_document() {
                            self.page = 0;
                        }
                    }
                    _ => {
                        self.bindings
                            .show(ui, &self.options.state.with_file_name("keybindings.json"));
                    }
                }
            });
        if let Some(session) = &mut self.session {
            self.dock.windows(
                ctx,
                session,
                self.options.light,
                self.appearance.terminal_size,
                &self.bindings,
            );
        }
        if !self.dock.message.is_empty() {
            self.status = std::mem::take(&mut self.dock.message);
        }
        if let Some(action) = self.dock.global_action.take() {
            match action.as_str() {
                "group_new_tab" => {
                    self.page = 1;
                    self.show_shell_picker = true;
                }
                "new_window" => self.new_window(),
                "terminal_focus" => {
                    self.page = 1;
                    self.focus_terminal = true;
                }
                "keybindings" => self.page = 6,
                "dashboard" => self.page = 0,
                _ => {}
            }
        }
        if let Some(s) = &mut self.session {
            self.dock.confirm(ctx, s);
        }
        let risk = self.session.as_ref().is_some_and(|s| self.dock.has_live(s))
            || self.editor.dirty()
            || self.launcher.has_unsaved_preview();
        if ctx.input(|i| i.viewport().close_requested())
            && !self.quit_approved
            && risk
            && !self.capturing()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.quit_pending = true;
        }
        if self.quit_pending {
            egui::Window::new("Close application?")
                .collapsible(false)
                .resizable(false)
                .frame(
                    egui::Frame::NONE
                        .inner_margin(24.0)
                        .fill(ctx.style().visuals.window_fill),
                )
                .show(ctx, |ui| {
                    ui.label("Running sessions will end. Unsaved editor changes will be lost.");
                    ui.horizontal(|ui| {
                        if crate::widgets::button(ui, "Quit and end sessions").clicked() {
                            self.quit_approved = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if crate::widgets::button(ui, "Keep working").clicked() {
                            self.quit_pending = false;
                        }
                    });
                });
        }
        crate::chrome::resize(ctx);
    }
}
fn collect_defaults(elements: &[Element], out: &mut BTreeMap<String, String>) {
    for e in elements {
        match e {
            Element::Input { id, value, .. } => {
                out.insert(id.clone(), value.clone());
            }
            Element::Stack { children, .. } => collect_defaults(children, out),
            Element::Place { child, .. } => {
                collect_defaults(std::slice::from_ref(child.as_ref()), out)
            }
            _ => {}
        }
    }
}
