//! Native GTK4 UI. GTK4 selects Wayland directly when WAYLAND_DISPLAY is set.
use grub_studio::{core, inspect};
use grub_studio::core::MenuEntry;
use grub_studio::inspect::{OsRecord, Snapshot, Theme};
use gtk4 as gtk;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const APP_ID: &str = "io.github.grubstudio.Universal";
const LANGUAGES: &[(&str, &str)] = &[
    ("pl", "Polski"), ("en", "English"), ("de", "Deutsch"), ("es", "Español"),
    ("fr", "Français"), ("uk", "Українська"), ("pt_BR", "Português (Brasil)"),
];

#[derive(Clone)]
struct StudioState {
    snapshot: Snapshot,
    authorized: bool,
    demo: bool,
    busy: bool,
    language: String,
    appearance: String,
    active: String,
    sidebar_visible: bool,
    selected_theme: String,
    selected_backup: Option<String>,
    selected_entry: Option<usize>,
    pending_entries: Vec<MenuEntry>,
    log: Vec<String>,
}
type Shared = Rc<RefCell<StudioState>>;

fn translate(lang: &str, key: &str) -> String {
    static TABLE: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
    let dictionary = TABLE.get_or_init(|| serde_json::from_str(include_str!("../assets/translations.json"))
        .expect("Bundled translations must be valid JSON"));
    dictionary.get(lang).and_then(|v| v.get(key)).and_then(|v| v.as_str())
        .or_else(|| dictionary.get("en").and_then(|v| v.get(key)).and_then(|v| v.as_str()))
        .unwrap_or(key).into()
}
fn tr(state: &Shared, key: &str) -> String { translate(&state.borrow().language, key) }
fn tf(state: &Shared, key: &str, args: &[(&str, &str)]) -> String {
    let mut value = tr(state, key);
    for (key, replacement) in args { value = value.replace(&format!("{{{key}}}"), replacement); }
    value
}
fn preferences_path() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/dev/null"));
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".config"))
        .join("bazzite-grub-studio/preferences.json")
}
fn load_prefs() -> (String, String) {
    let locale = std::env::var("LC_ALL").or_else(|_| std::env::var("LANG")).unwrap_or_else(|_| "en".into());
    let prefix = locale.split(|c| c == '_' || c == '-' || c == '.').next().unwrap_or("en");
    let default_lang = if locale.starts_with("pt") { "pt_BR" } else if LANGUAGES.iter().any(|(id, _)| *id == prefix) { prefix } else { "en" };
    let mut result = ("dark".to_string(), default_lang.to_string());
    let path = preferences_path();
    if let Ok(meta) = fs::symlink_metadata(&path) {
        if !meta.file_type().is_symlink() && meta.len() < 8192 {
            if let Ok(bytes) = fs::read(&path) {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    let mode = json.get("appearance").and_then(|v| v.as_str()).unwrap_or("");
                    let lang = json.get("language").and_then(|v| v.as_str()).unwrap_or("");
                    if matches!(mode, "dark" | "light" | "system") { result.0 = mode.to_string(); }
                    if LANGUAGES.iter().any(|(id, _)| *id == lang) { result.1 = lang.to_string(); }
                }
            }
        }
    }
    result
}
fn save_prefs(s: &StudioState) {
    let path = preferences_path();
    if let Some(parent) = path.parent() {
        if fs::create_dir_all(parent).is_err() { return; }
        if fs::symlink_metadata(parent).map(|m| m.file_type().is_symlink()).unwrap_or(true) { return; }
        if fs::symlink_metadata(&path).map(|m| m.file_type().is_symlink()).unwrap_or(false) { return; }
        let temp = parent.join(format!(".preferences-{}-{}.tmp", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()));
        let content = serde_json::json!({"appearance":s.appearance, "language":s.language}).to_string();
        if fs::write(&temp, content).is_ok() { let _ = fs::rename(temp, path); }
    }
}
fn demo_snapshot() -> Snapshot {
    let mut snapshot = inspect::inspect();
    snapshot.profile.name = "Bazzite KDE (demo)".into();
    snapshot.profile.writable = true;
    snapshot.profile.reasons.clear();
    snapshot.profile.family = core::Family::Bazzite;
    snapshot.config.insert("GRUB_TIMEOUT".into(), "5".into());
    snapshot.config.insert("GRUB_TIMEOUT_STYLE".into(), "menu".into());
    snapshot.config.insert("GRUB_GFXMODE".into(), "1920x1080".into());
    snapshot.config.insert("GRUB_THEME".into(), "/boot/grub2/themes/legion/theme.txt".into());
    snapshot.themes = vec![
        Theme {name:"legion".into(), path:"/boot/grub2/themes/legion/theme.txt".into()},
        Theme {name:"lenovo".into(), path:"/boot/grub2/themes/lenovo/theme.txt".into()},
        Theme {name:"asus-rog".into(), path:"/boot/grub2/themes/asus-rog/theme.txt".into()},
    ];
    snapshot.systems = vec![
        OsRecord {name:"Windows Boot Manager".into(), source:"UEFI".into(),
            detail:"Boot0001* Windows Boot Manager".into()},
        OsRecord {name:"Fedora (Bazzite)".into(), source:"UEFI".into(),
            detail:"Boot0002* Fedora (GRUB)".into()},
    ];
    snapshot.deployments = vec!["0 · bazzite · 44.20261113 (booted)".into(), "1 · bazzite · 44.20261106".into()];
    snapshot.backups = vec!["20261113-130311-1024".into(), "20261112-214500-1124".into()];
    snapshot.menu = vec![
        MenuEntry {id:"gs-0123456789ab".into(), title:"Windows 11".into(), uuid:"9A4C-10EF".into(),
            path:"/EFI/Microsoft/Boot/bootmgfw.efi".into(), enabled:true},
    ];
    snapshot.generated = vec!["Bazzite Linux".into(), "Bazzite Linux (rollback)".into(), "Windows Boot Manager".into()];
    snapshot
}
fn run_admin(args: &[String]) -> Result<String, String> {
    let pkexec = inspect::program_path("pkexec").ok_or("Polkit / pkexec is missing")?;
    let exe = std::env::current_exe().map_err(|e| format!("Cannot locate application executable: {e}"))?;
    let helper = exe.parent().ok_or("Invalid application path")?.join("grub-studio-helper");
    if !helper.is_file() { return Err(format!("Missing separate helper executable: {}", helper.display())); }
    let result = Command::new(pkexec).arg(helper).args(args).output().map_err(|e| format!("Polkit error: {e}"))?;
    if result.status.success() { Ok(String::from_utf8_lossy(&result.stdout).into_owned()) }
    else if result.status.code() == Some(126) { Err("Administrator authentication cancelled".into()) }
    else {
        let stderr = String::from_utf8_lossy(&result.stderr);
        Err(format!("Operation denied or failed: {}", stderr.chars().take(1800).collect::<String>()))
    }
}
fn apply_probe(s: &mut StudioState, raw: &str, deep: bool) -> Result<(), String> {
    let header = if deep { "GRUB_STUDIO_DEEP_V2" } else { "GRUB_STUDIO_PROBE_V2" };
    if raw.lines().next() != Some(header) { return Err("Invalid privileged scan output".into()); }
    if !deep {
        s.snapshot.themes.clear();
        s.snapshot.systems.clear();
        s.snapshot.backups.clear();
        s.snapshot.menu.clear();
        s.snapshot.generated.clear();
        s.snapshot.deployments.clear();
        s.snapshot.config.clear();
    }
    for line in raw.lines().skip(1) {
        let p: Vec<&str> = line.split('\t').collect();
        match p.as_slice() {
            ["PROFILE", family, writable, name, reason] if !deep => {
                s.snapshot.profile.writable = *writable == "1";
                s.snapshot.profile.name = (*name).to_string();
                s.snapshot.profile.reasons = if reason.is_empty() { vec![] } else { vec![(*reason).to_string()] };
                s.snapshot.profile.family = match *family {
                    "bazzite" => core::Family::Bazzite, "fedora" => core::Family::Fedora,
                    "debian" => core::Family::Debian, "suse" => core::Family::Suse,
                    "arch" => core::Family::Arch, _ => core::Family::ReadOnly,
                };
            }
            ["CONFIG", key, val] if !deep => { s.snapshot.config.insert((*key).into(), (*val).into()); }
            ["THEME", name, path] if !deep => { s.snapshot.themes.push(Theme {name:(*name).into(),path:(*path).into()}); }
            ["OS", name, source, detail] => {
                let record = OsRecord {name:(*name).into(),source:(*source).into(),detail:(*detail).into()};
                if !s.snapshot.systems.contains(&record) { s.snapshot.systems.push(record); }
            }
            ["BACKUP", name] if !deep => { s.snapshot.backups.push((*name).into()); }
            ["DEPLOY", info] if !deep => { s.snapshot.deployments.push((*info).into()); }
            ["ENTRY", id, title, uuid, path, enabled] if !deep => {
                s.snapshot.menu.push(MenuEntry {id:(*id).into(),title:(*title).into(),uuid:(*uuid).into(),
                    path:(*path).into(),enabled:*enabled=="1"});
            }
            ["GENERATED", title] if !deep => { s.snapshot.generated.push((*title).into()); }
            _ => {}
        }
    }
    if !deep {
        s.pending_entries = s.snapshot.menu.clone();
        s.selected_entry = None;
        s.selected_theme = s.snapshot.config.get("GRUB_THEME").cloned().unwrap_or_default();
    }
    Ok(())
}
fn initial_state(demo: bool) -> StudioState {
    let prefs = load_prefs();
    let mut s = StudioState {
        snapshot: if demo { demo_snapshot() } else { inspect::inspect() },
        authorized: false, demo, busy: false, language: prefs.1, appearance: prefs.0,
        active:"home".into(), sidebar_visible:true, selected_theme:String::new(),
        selected_backup:None, selected_entry:None, pending_entries:vec![], log:vec![],
    };
    if demo {
        s.authorized = true;
        s.pending_entries = s.snapshot.menu.clone();
        s.selected_theme = s.snapshot.config.get("GRUB_THEME").cloned().unwrap_or_default();
        s.log.push("DEMO: all administrative writes are disabled".into());
    } else {
        // Required by the original brief: system Polkit password dialog on startup.
        // The GUI never receives, stores or displays the password.
        match run_admin(&["--probe".into()]) {
            Ok(text) => match apply_probe(&mut s, &text, false) {
                Ok(()) => { s.authorized = true; s.log.push("Polkit authorization succeeded".into()); }
                Err(e) => s.log.push(format!("Root scan error: {e}")),
            },
            Err(e) => s.log.push(format!("Read-only session: {e}")),
        }
    }
    s
}
fn can_edit(s: &StudioState) -> bool {
    s.authorized && !s.demo && s.snapshot.profile.writable && !s.busy
}
fn show_message(window: &gtk::ApplicationWindow, title: &str, body: &str, error: bool) {
    let dialog = gtk::MessageDialog::builder()
        .transient_for(window).modal(true)
        .message_type(if error { gtk::MessageType::Error } else { gtk::MessageType::Info })
        .buttons(gtk::ButtonsType::Ok).text(title).secondary_text(body).build();
    dialog.connect_response(|dialog, _| dialog.close());
    dialog.present();
}
fn confirm(window: &gtk::ApplicationWindow, title: &str, message: &str, action: impl FnOnce() + 'static) {
    let dialog = gtk::MessageDialog::builder()
        .transient_for(window).modal(true).message_type(gtk::MessageType::Warning)
        .buttons(gtk::ButtonsType::YesNo).text(title).secondary_text(message).build();
    let f = Rc::new(RefCell::new(Some(action)));
    dialog.connect_response(move |dialog, response| {
        dialog.close();
        if response == gtk::ResponseType::Yes {
            if let Some(f) = f.borrow_mut().take() { f(); }
        }
    });
    dialog.present();
}
fn submit(window: &gtk::ApplicationWindow, shared: &Shared, args: Vec<String>, deep: bool) {
    if shared.borrow().busy { return; }
    if shared.borrow().demo {
        show_message(window, "Demo", "Preview mode never writes to the system.", false);
        return
    }
    if !shared.borrow().authorized {
        show_message(window, "Polkit", "Restart the application and approve administrator authentication.", true);
        return
    }
    {
        let mut s = shared.borrow_mut();
        s.busy = true;
        s.log.push(format!("START: {}", args.first().cloned().unwrap_or_default()));
    }
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = run_admin(&args).and_then(|output| {
            if deep { Ok((output, None)) }
            else if args.first().map(String::as_str) == Some("--probe") {
                Ok(("System scan complete".into(), Some(output)))
            }
            else {
                let refreshed = run_admin(&["--probe".into()]).ok();
                Ok((output, refreshed))
            }
        });
        let _ = tx.send(result);
    });
    let window = window.clone();
    let state = shared.clone();
    glib::timeout_add_local(Duration::from_millis(100), move || match rx.try_recv() {
        Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        result => {
            let response = match result {
                Ok(Ok((message, scan))) => {
                    let mut s = state.borrow_mut();
                    if deep {
                        if let Err(e) = apply_probe(&mut s, &message, true) { s.log.push(e); }
                    } else if let Some(scan) = scan {
                        if let Err(e) = apply_probe(&mut s, &scan, false) { s.log.push(e); }
                    }
                    s.log.push(message.chars().take(1400).collect());
                    Ok(message)
                }
                Ok(Err(e)) => { state.borrow_mut().log.push(e.clone()); Err(e) }
                Err(_) => Err("Administrative worker stopped unexpectedly".into()),
            };
            state.borrow_mut().busy = false;
            rebuild(&window, &state);
            match response {
                Ok(info) => if !deep { show_message(&window, "GRUB Studio", &info, false); },
                Err(err) => show_message(&window, "GRUB Studio", &err, true),
            }
            glib::ControlFlow::Break
        }
    });
}
fn guarded_submit(window: &gtk::ApplicationWindow, state: &Shared, args: Vec<String>, note: String) {
    let w = window.clone();
    let s = state.clone();
    confirm(window, &tr(state, "dialog.confirm"),
        &format!("{}\n\n{}", note, tr(state, "dialog.risk")), move || submit(&w, &s, args, false));
}

fn add_text(parent: &gtk::Box, text: impl AsRef<str>, class: &str, wrap: bool) -> gtk::Label {
    let label = gtk::Label::new(Some(text.as_ref()));
    label.set_xalign(0.0);
    label.set_wrap(wrap);
    label.add_css_class(class);
    parent.append(&label);
    label
}
fn button(label: &str, primary: bool) -> gtk::Button {
    let b = gtk::Button::with_label(label);
    b.add_css_class(if primary { "suggested-action" } else { "flat" });
    b.add_css_class("action-button");
    b
}
fn action_row() -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.add_css_class("actions");
    row
}
fn card() -> gtk::Box {
    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 10);
    box_.add_css_class("studio-card");
    box_
}
fn section(parent: &gtk::Box, heading: &str) -> gtk::Box {
    let content = card();
    add_text(&content, heading, "section-heading", true);
    parent.append(&content);
    content
}
fn page_root(title: &str, subtitle: &str) -> gtk::Box {
    let v = gtk::Box::new(gtk::Orientation::Vertical, 14);
    v.set_margin_top(20); v.set_margin_bottom(26);
    v.set_margin_start(20); v.set_margin_end(20);
    add_text(&v, title, "page-title", true);
    add_text(&v, subtitle, "muted", true);
    v
}
fn row_with_heading(title: &str, subtitle: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Vertical, 3);
    add_text(&row, title, "row-title", true);
    add_text(&row, subtitle, "muted", true);
    row
}
fn form_row(parent: &gtk::Box, name: &str, widget: &impl IsA<gtk::Widget>) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let label = gtk::Label::new(Some(name));
    label.set_xalign(0.0); label.set_hexpand(true); label.set_wrap(true);
    row.append(&label);
    widget.set_hexpand(true);
    widget.set_size_request(135, -1);
    row.append(widget);
    parent.append(&row);
}

fn page_home(window: &gtk::ApplicationWindow, state: &Shared) -> gtk::Box {
    let snapshot = state.borrow().snapshot.clone();
    let page = page_root(&tr(state, "home.title"), &tr(state, "home.sub"));
    let banner = card();
    banner.add_css_class("hero");
    add_text(&banner, &snapshot.profile.name, "hero-title", true);
    add_text(&banner, &format!("{} · GRUB 2 · {}",
        snapshot.profile.family.label(),
        if snapshot.profile.writable { tr(state, "status.ready") } else { tr(state, "status.readonly") }),
        "muted", true);
    if !snapshot.profile.writable {
        add_text(&banner, &snapshot.profile.reasons.join(" • "), "warning", true);
    }
    let actions = action_row();
    let go_settings = button(&tr(state, "home.change"), true);
    let s = state.clone(); let w = window.clone();
    go_settings.connect_clicked(move |_| { s.borrow_mut().active = "settings".into(); rebuild(&w,&s); });
    actions.append(&go_settings);
    let refresh = button(&tr(state, "home.refresh"), false);
    let s = state.clone(); let w = window.clone();
    refresh.connect_clicked(move |_| submit(&w, &s, vec!["--probe".into()], false));
    actions.append(&refresh);
    banner.append(&actions);
    page.append(&banner);
    let overview = section(&page, &tr(state, "home.deployments"));
    add_text(&overview, &format!("{}: {}", tr(state,"home.theme"),
        snapshot.config.get("GRUB_THEME").map(|s| s.rsplit('/').nth(1).unwrap_or(s)).unwrap_or("—")), "row-title", true);
    add_text(&overview, &format!("{}: {}", tr(state, "home.others"), snapshot.systems.len()), "muted", true);
    for dep in &snapshot.deployments { add_text(&overview, dep, "item-label", true); }
    if snapshot.deployments.is_empty() { add_text(&overview, &tr(state,"home.deployments.sub"), "muted", true); }
    let warning = section(&page, &tr(state, "home.warning.title"));
    add_text(&warning, &tr(state,"home.warning.text"), "warning", true);
    page
}

fn page_systems(window: &gtk::ApplicationWindow, state: &Shared) -> gtk::Box {
    let page = page_root(&tr(state,"systems.title"), &tr(state,"systems.sub"));
    let actions = action_row();
    let quick = button(&tr(state,"systems.quick"), true);
    let w=window.clone(); let s=state.clone();
    quick.connect_clicked(move |_| submit(&w, &s, vec!["--probe".into()], false));
    actions.append(&quick);
    let deep = button(&tr(state,"systems.deep"), false);
    let w=window.clone(); let s=state.clone();
    deep.connect_clicked(move |_| {
        let w2=w.clone(); let s2=s.clone();
        confirm(&w, &tr(&s,"systems.deep.title"), &tr(&s,"systems.deep.ask"),
            move || submit(&w2, &s2, vec!["--scan-deep".into()], true));
    });
    actions.append(&deep);
    page.append(&actions);
    let list = section(&page, &tr(state, "systems.results"));
    let items=state.borrow().snapshot.systems.clone();
    if items.is_empty() { add_text(&list, &tr(state,"systems.empty"), "muted", true); }
    for item in items {
        let row=row_with_heading(&item.name, &format!("{} · {}",item.source,item.detail));
        row.add_css_class("list-row");
        list.append(&row);
    }
    add_text(&page, &tr(state,"systems.footnote"), "muted", true);
    page
}

fn entry_with_text(value: &str) -> gtk::Entry {
    let entry = gtk::Entry::new();
    entry.set_text(value);
    entry
}
fn page_settings(window: &gtk::ApplicationWindow, state: &Shared) -> gtk::Box {
    let page=page_root(&tr(state,"settings.title"), &tr(state,"settings.sub"));
    let config=state.borrow().snapshot.config.clone();
    let writable=can_edit(&state.borrow());
    let grid=section(&page,&tr(state,"settings.general"));
    let timeout=entry_with_text(config.get("GRUB_TIMEOUT").map(String::as_str).unwrap_or("5"));
    form_row(&grid, &tr(state,"settings.timeout"), &timeout);
    let styles=["menu","hidden","countdown"];
    let style=gtk::DropDown::from_strings(&styles);
    style.set_selected(styles.iter().position(|x| Some(*x)==config.get("GRUB_TIMEOUT_STYLE").map(String::as_str)).unwrap_or(0) as u32);
    form_row(&grid,&tr(state,"settings.style"),&style);
    let gfx=entry_with_text(config.get("GRUB_GFXMODE").map(String::as_str).unwrap_or("auto"));
    form_row(&grid,&tr(state,"settings.resolution"),&gfx);
    let themes=state.borrow().snapshot.themes.clone();
    let mut choices=vec![tr(state, "settings.none")];
    choices.extend(themes.iter().map(|x| x.name.clone()));
    let refs:Vec<&str>=choices.iter().map(String::as_str).collect();
    let theme=gtk::DropDown::from_strings(&refs);
    let active=state.borrow().selected_theme.clone();
    theme.set_selected(themes.iter().position(|x| x.path==active).map(|x| x+1).unwrap_or(0) as u32);
    form_row(&grid,&tr(state,"settings.theme"),&theme);
    add_text(&grid,&tr(state,"settings.timeouthelp"),"muted",true);
    let row=action_row();
    let save=button(&tr(state,"settings.save"),true);
    save.set_sensitive(writable);
    let w=window.clone(); let s=state.clone();
    save.connect_clicked(move |_| {
        let timeout=timeout.text().to_string();
        let gfx=gfx.text().to_string();
        let style=styles.get(style.selected() as usize).unwrap_or(&"menu").to_string();
        let theme=themes.get(theme.selected() as usize - if theme.selected()>0 {1}else{0})
            .filter(|_| theme.selected()>0).map(|t| t.path.clone()).unwrap_or_default();
        if !core::valid_timeout(&timeout) || !core::valid_style(&style) || !core::valid_resolution(&gfx)
            || !core::valid_theme_path(&theme) {
            show_message(&w, &tr(&s,"dialog.invalid"), "Invalid timeout, resolution or theme path.", true); return;
        }
        guarded_submit(&w,&s,vec!["--apply".into(),timeout,style,gfx,theme],tr(&s,"dialog.applysettings"));
    });
    row.append(&save);
    let reset=button(&tr(state,"settings.reload"),false);
    let w=window.clone(); let s=state.clone();
    reset.connect_clicked(move |_| submit(&w,&s,vec!["--probe".into()],false));
    row.append(&reset);
    page.append(&row);
    let visibility=section(&page,&tr(state,"settings.visibility"));
    add_text(&visibility,&tr(state,"settings.visibility.note"),"muted",true);
    let actions=action_row();
    for (id,key) in [("show","settings.visibility.show"),("auto","settings.visibility.auto"),("hide","settings.visibility.hide")] {
        let b=button(&tr(state,key),false);
        b.set_sensitive(writable && matches!(state.borrow().snapshot.profile.family,core::Family::Bazzite|core::Family::Fedora));
        let w=window.clone(); let s=state.clone();
        b.connect_clicked(move |_| guarded_submit(&w,&s,vec!["--visibility".into(),id.into()],tr(&s,"dialog.applyvisibility")));
        actions.append(&b);
    }
    visibility.append(&actions);
    let kernel=section(&page,&tr(state,"menu.kernel"));
    add_text(&kernel,&tr(state,"menu.kernel.note"),"muted",true);
    let kargs=entry_with_text(config.get("GRUB_CMDLINE_LINUX_DEFAULT").map(String::as_str).unwrap_or(""));
    kernel.append(&kargs);
    let save_kernel=button(&tr(state,"menu.kernel.save"),false);
    save_kernel.set_sensitive(writable && matches!(state.borrow().snapshot.profile.family,core::Family::Arch|core::Family::Debian));
    let w=window.clone(); let s=state.clone();
    save_kernel.connect_clicked(move |_| {
        let value=kargs.text().to_string();
        if !core::valid_kernel_args(&value) { show_message(&w,"GRUB","Invalid kernel parameters",true); return; }
        guarded_submit(&w,&s,vec!["--kernel-args".into(),value.clone()],
            tf(&s,"menu.kernel.confirm",&[("args",&value)]));
    });
    kernel.append(&save_kernel);
    page
}

fn new_id() -> String {
    let nanos=SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("gs-{:012x}", (nanos ^ (std::process::id() as u128)) & 0xffffffffffff)
}
fn page_menu(window: &gtk::ApplicationWindow, state: &Shared) -> gtk::Box {
    let page=page_root(&tr(state,"menu.title"),&tr(state,"menu.sub"));
    let profile=state.borrow().snapshot.profile.clone();
    let editable=can_edit(&state.borrow()) && profile.family.supports_menu_edit();
    let info=section(&page,&tr(state,"menu.custom"));
    add_text(&info,&tr(state,"menu.custom.note"),"muted",true);
    if !profile.family.supports_menu_edit() { add_text(&info,&tr(state,"menu.readonly"),"warning",true); }
    let list=gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.add_css_class("studio-list");
    let items=state.borrow().pending_entries.clone();
    if items.is_empty() { add_text(&info,&tr(state,"menu.empty"),"muted",true); }
    for item in &items {
        let label=row_with_heading(&item.title,&format!("{} · {} · {}",item.uuid,item.path,
            if item.enabled {tr(state,"menu.enabled")} else {tr(state,"menu.hidden")}));
        label.set_margin_top(7); label.set_margin_bottom(7); label.set_margin_start(11); label.set_margin_end(11);
        list.append(&label);
    }
    info.append(&list);
    let fields=gtk::Box::new(gtk::Orientation::Vertical,7);
    let title=entry_with_text(""); title.set_placeholder_text(Some(&tr(state,"menu.field.title")));
    let uuid=entry_with_text(""); uuid.set_placeholder_text(Some(&tr(state,"menu.field.uuid")));
    let path=entry_with_text("/EFI/Microsoft/Boot/bootmgfw.efi");
    fields.append(&title);fields.append(&uuid);fields.append(&path);
    info.append(&fields);
    let s=state.clone();
    let title2=title.clone(); let uuid2=uuid.clone(); let path2=path.clone();
    list.connect_row_selected(move |_, row| {
        if let Some(index)=row.map(|r| r.index() as usize) {
            s.borrow_mut().selected_entry=Some(index);
            if let Some(item)=s.borrow().pending_entries.get(index) {
                title2.set_text(&item.title); uuid2.set_text(&item.uuid); path2.set_text(&item.path);
            }
        }
    });
    let selected_index=state.borrow().selected_entry;
    if let Some(index)=selected_index {
        if let Some(row)=list.row_at_index(index as i32) { list.select_row(Some(&row)); }
    }
    let controls=action_row();
    let add=button(&tr(state,"menu.add"),false);
    let w=window.clone();let s=state.clone();
    let title1=title.clone();let uuid1=uuid.clone();let path1=path.clone();
    add.connect_clicked(move |_| {
        let item=MenuEntry {id:new_id(),title:title1.text().to_string(),uuid:uuid1.text().to_string(),
            path:path1.text().to_string(),enabled:true};
        if !core::valid_entry(&item) { show_message(&w,"GRUB Studio","Invalid EFI entry title, UUID or path",true);return; }
        s.borrow_mut().pending_entries.push(item);
        s.borrow_mut().selected_entry=None;
        rebuild(&w,&s);
    });
    controls.append(&add);
    let edit=button(&tr(state,"menu.edit"),false);
    let w=window.clone();let s=state.clone();
    edit.connect_clicked(move |_| {
        let idx=s.borrow().selected_entry;
        let Some(idx)=idx else { return };
        let Some(mut item)=s.borrow().pending_entries.get(idx).cloned() else {return};
        item.title=title.text().to_string();
        item.uuid=uuid.text().to_string();
        item.path=path.text().to_string();
        if !core::valid_entry(&item) { show_message(&w,"GRUB Studio","Invalid EFI entry",true);return; }
        s.borrow_mut().pending_entries[idx]=item;
        rebuild(&w,&s);
    });
    controls.append(&edit);
    for (key,operation) in [("menu.up",-1i32),("menu.toggle",0)] {
        let b=button(&tr(state,key),false);
        let w=window.clone();let s=state.clone();
        b.connect_clicked(move |_| {
            let idx=s.borrow().selected_entry;
            let Some(idx)=idx else{return};
            let mut data=s.borrow_mut();
            if idx>=data.pending_entries.len(){return}
            match operation {
                -1 if idx>0 => { data.pending_entries.swap(idx,idx-1);data.selected_entry=Some(idx-1); }
                1 if idx+1<data.pending_entries.len() => {data.pending_entries.swap(idx,idx+1);data.selected_entry=Some(idx+1);}
                0 => data.pending_entries[idx].enabled=!data.pending_entries[idx].enabled,
                2 => {data.pending_entries.remove(idx);data.selected_entry=None;},
                _ => {}
            }
            drop(data);
            rebuild(&w,&s);
        });
        controls.append(&b);
    }
    info.append(&controls);
    let controls2=action_row();
    for (key,operation) in [("menu.down",1i32),("menu.remove",2)] {
        let b=button(&tr(state,key),false);
        let w=window.clone();let s=state.clone();
        b.connect_clicked(move |_| {
            let Some(idx)=s.borrow().selected_entry else {return};
            let mut data=s.borrow_mut();
            if idx>=data.pending_entries.len(){return}
            match operation {
                1 if idx+1<data.pending_entries.len() => {data.pending_entries.swap(idx,idx+1);data.selected_entry=Some(idx+1);}
                2 => {data.pending_entries.remove(idx);data.selected_entry=None;},
                _ => {}
            }
            drop(data);rebuild(&w,&s);
        });
        controls2.append(&b);
    }
    info.append(&controls2);
    let commit=action_row();
    let save=button(&tr(state,"menu.save"),true);
    save.set_sensitive(editable);
    let w=window.clone();let s=state.clone();
    save.connect_clicked(move |_| {
        let entries=s.borrow().pending_entries.clone();
        match core::entries_to_tsv(&entries) {
            Ok(rows) => guarded_submit(&w,&s,vec!["--menu-update".into(),core::hex_encode(&rows)],tr(&s,"menu.save.confirm")),
            Err(e) => show_message(&w,"GRUB Studio",&e,true),
        }
    });
    commit.append(&save);
    let set_default=button(&tr(state,"menu.defaultcustom"),false);
    set_default.set_sensitive(editable);
    let w=window.clone();let s=state.clone();
    set_default.connect_clicked(move |_| {
        let chosen=s.borrow().selected_entry.and_then(|i| s.borrow().pending_entries.get(i).cloned());
        let Some(item)=chosen else {show_message(&w,"GRUB Studio",&tr(&s,"menu.selectfirst"),true);return};
        guarded_submit(&w,&s,vec!["--default".into(),item.id.clone()],
            tf(&s,"menu.default.confirm",&[("id",&item.id)]));
    });
    let secondary=action_row();
    secondary.append(&set_default);
    let preview=button(&tr(state,"menu.preview"),false);
    let w=window.clone();let s=state.clone();
    preview.connect_clicked(move |_| {
        match core::render_grub_entries(&s.borrow().pending_entries) {
            Ok(script)=>show_message(&w,&tr(&s,"menu.preview"),&script,false),
            Err(e)=>show_message(&w,"GRUB Studio",&e,true),
        }
    });
    secondary.append(&preview);
    let discard=button(&tr(state,"menu.discard"),false);
    let w=window.clone();let s=state.clone();
    discard.connect_clicked(move |_| {
        let original=s.borrow().snapshot.menu.clone();
        {
            let mut data=s.borrow_mut();
            data.pending_entries=original;
            data.selected_entry=None;
        }
        rebuild(&w,&s);
    });
    secondary.append(&discard);
    page.append(&commit);
    page.append(&secondary);
    let generated=section(&page,&tr(state,"menu.detected"));
    add_text(&generated,&tr(state,"menu.detected.note"),"muted",true);
    let titles=state.borrow().snapshot.generated.clone();
    if titles.is_empty() {add_text(&generated,&tr(state,"menu.empty"),"muted",true);}
    for (index,title) in titles.iter().enumerate() {
        add_text(&generated,&format!("{}  {}",index+1,title),"item-label",true);
    }
    add_text(&page,&tr(state,"menu.efiwarn"),"muted",true);
    page
}

fn page_themes(window: &gtk::ApplicationWindow, state: &Shared) -> gtk::Box {
    let page=page_root(&tr(state,"themes.title"),&tr(state,"themes.sub"));
    let list=section(&page,&tr(state,"themes.installed"));
    let selected=state.borrow().selected_theme.clone();
    let themes=state.borrow().snapshot.themes.clone();
    if themes.is_empty(){add_text(&list,&tr(state,"themes.placeholder"),"muted",true);}
    let selector=gtk::ListBox::new();
    selector.add_css_class("studio-list");
    for theme in &themes {
        let display=if selected==theme.path {format!("●  {}",theme.name)} else {format!("○  {}",theme.name)};
        let row=row_with_heading(&display,&theme.path);
        row.set_margin_top(6); row.set_margin_bottom(6); row.set_margin_start(9);row.set_margin_end(9);
        selector.append(&row);
    }
    let s=state.clone();
    selector.connect_row_selected(move |_,row| {
        if let Some(row)=row {
            if let Some(path)=themes.get(row.index() as usize) {s.borrow_mut().selected_theme=path.path.clone();}
        }
    });
    list.append(&selector);
    let row=action_row();
    let install=button(&tr(state,"themes.import"),true);
    install.set_sensitive(can_edit(&state.borrow()));
    let w=window.clone();let s=state.clone();
    install.connect_clicked(move |_| {
        let dialog=gtk::FileChooserNative::new(Some(&tr(&s,"themes.import")),Some(&w),
            gtk::FileChooserAction::SelectFolder,Some("Select"),Some("Cancel"));
        // NativeDialog is not a toplevel widget; keep it alive until response.
        let dialog_hold=dialog.clone();
        let w2=w.clone();let s2=s.clone();
        dialog.connect_response(move |_dialog,response| {
            if response==gtk::ResponseType::Accept {
                if let Some(path)=dialog.file().and_then(|file|file.path()) {
                    let folder=path.file_name().map(|x|x.to_string_lossy().to_string()).unwrap_or_default();
                    if !core::valid_simple_name(&folder) {show_message(&w2,"GRUB Studio","Theme folder must have an ASCII alphanumeric name",true);}
                    else {guarded_submit(&w2,&s2,vec!["--install-theme".into(),path.to_string_lossy().to_string(),folder],tr(&s2,"themes.import"));}
                }
            }
            dialog_hold.destroy();
        });
        dialog.show();
    });
    row.append(&install);
    let activate=button(&tr(state,"themes.use"),false);
    activate.set_sensitive(can_edit(&state.borrow()));
    let w=window.clone();let s=state.clone();
    activate.connect_clicked(move |_| {
        let c=s.borrow().snapshot.config.clone();let selected=s.borrow().selected_theme.clone();
        if !core::valid_theme_path(&selected){show_message(&w,"GRUB Studio","Select an installed theme",true);return;}
        guarded_submit(&w,&s,vec!["--apply".into(),
            c.get("GRUB_TIMEOUT").cloned().unwrap_or_else(||"5".into()),
            c.get("GRUB_TIMEOUT_STYLE").cloned().unwrap_or_else(||"menu".into()),
            c.get("GRUB_GFXMODE").cloned().unwrap_or_else(||"auto".into()),selected],tr(&s,"themes.use"));
    });
    row.append(&activate);
    let disable=button(&tr(state,"themes.disable"),false);
    disable.set_sensitive(can_edit(&state.borrow()));
    let w=window.clone();let s=state.clone();
    disable.connect_clicked(move |_| {
        let c=s.borrow().snapshot.config.clone();
        guarded_submit(&w,&s,vec!["--apply".into(),
            c.get("GRUB_TIMEOUT").cloned().unwrap_or_else(||"5".into()),
            c.get("GRUB_TIMEOUT_STYLE").cloned().unwrap_or_else(||"menu".into()),
            c.get("GRUB_GFXMODE").cloned().unwrap_or_else(||"auto".into()),String::new()],tr(&s,"themes.disable"));
    });
    row.append(&disable);
    list.append(&row);
    let preview=section(&page,&tr(state,"themes.preview"));
    let canvas=gtk::Box::new(gtk::Orientation::Vertical,8);
    canvas.add_css_class("boot-preview");
    add_text(&canvas,"GNU GRUB 2", "preview-title",true);
    for (i,entry) in ["Bazzite Linux", "Advanced options for Linux", "Windows Boot Manager"].iter().enumerate() {
        let l=gtk::Label::new(Some(entry));
        l.set_xalign(0.0);l.add_css_class(if i==0{"preview-selected"}else{"preview-entry"});
        canvas.append(&l);
    }
    preview.append(&canvas);
    add_text(&preview,&format!("{}\n{}",selected,tr(state,"themes.approx")),"muted",true);
    page
}

fn page_backups(window: &gtk::ApplicationWindow, state: &Shared) -> gtk::Box {
    let page=page_root(&tr(state,"backups.title"),&tr(state,"backups.sub"));
    let controls=action_row();
    let create=button(&tr(state,"backups.create"),true);
    create.set_sensitive(can_edit(&state.borrow()));
    let w=window.clone();let s=state.clone();
    create.connect_clicked(move |_| submit(&w,&s,vec!["--backup".into()],false));
    controls.append(&create);
    let restore=button(&tr(state,"backups.restore"),false);
    restore.set_sensitive(can_edit(&state.borrow()));
    let w=window.clone();let s=state.clone();
    restore.connect_clicked(move |_| {
        let Some(name)=s.borrow().selected_backup.clone() else {
            show_message(&w,"GRUB Studio",&tr(&s,"dialog.selectbackup"),true);return;
        };
        guarded_submit(&w,&s,vec!["--restore".into(),name.clone()],tf(&s,"dialog.restore",&[("name",&name)]));
    });
    controls.append(&restore);
    page.append(&controls);
    let box_=section(&page,&tr(state,"backups.saved"));
    let backups=state.borrow().snapshot.backups.clone();
    if backups.is_empty() {add_text(&box_,&tr(state,"backups.empty"),"muted",true);}
    let list=gtk::ListBox::new();
    list.add_css_class("studio-list");
    for backup in &backups {
        let label=row_with_heading(backup,"/var/lib/…/backups/");
        label.set_margin_top(8);label.set_margin_bottom(8);label.set_margin_start(11);label.set_margin_end(11);
        list.append(&label);
    }
    let s=state.clone();
    list.connect_row_selected(move |_,r| {
        if let Some(r)=r {s.borrow_mut().selected_backup=backups.get(r.index() as usize).cloned();}
    });
    box_.append(&list);
    add_text(&page,&tr(state,"backups.note"),"muted",true);
    page
}
fn page_logs(_window: &gtk::ApplicationWindow, state: &Shared) -> gtk::Box {
    let page=page_root(&tr(state,"logs.title"),&tr(state,"logs.sub"));
    let row=action_row();
    let copy=button(&tr(state,"logs.copy"),false);
    let log=state.borrow().log.join("\n\n");
    let text=log.clone();
    copy.connect_clicked(move |_| {if let Some(display)=gdk::Display::default(){ display.clipboard().set_text(&text);}});
    row.append(&copy);page.append(&row);
    let box_=section(&page,"");
    let textview=gtk::TextView::new();
    textview.set_monospace(true);textview.set_editable(false);textview.set_cursor_visible(false);
    textview.set_wrap_mode(gtk::WrapMode::WordChar);
    textview.buffer().set_text(&log);
    textview.set_vexpand(true);
    box_.append(&textview);
    page
}
fn page_about(_window: &gtk::ApplicationWindow, state: &Shared) -> gtk::Box {
    let page=page_root(&tr(state,"about.title"),"Rust Edition · GTK4 · Wayland · 2.0.0");
    for number in 1..=9 {
        let title=tr(state,&format!("about.{number:02}.title"));
        let text=tr(state,&format!("about.{number:02}.text"));
        let section=section(&page,&title);
        add_text(&section,&text,"muted",true);
    }
    add_text(&page,"GPL-3.0-or-later · Independent application · GTK4 runtime required", "muted",true);
    page
}

fn load_css() {
    let provider=gtk::CssProvider::new();
    // load_from_data is available on GTK4 before 4.12; keep older desktop support.
    provider.load_from_data(include_str!("../assets/style.css"));
    if let Some(display)=gdk::Display::default(){
        gtk::style_context_add_provider_for_display(&display,&provider,gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
}
fn rebuild(window: &gtk::ApplicationWindow, state: &Shared) {
    let s=state.borrow().clone();
    let root=gtk::Box::new(gtk::Orientation::Vertical,0);
    root.add_css_class("app-root");
    root.add_css_class(if s.appearance=="light" { "grub-light" } else { "grub-dark" });
    root.set_size_request(600,410);
    let header=gtk::Box::new(gtk::Orientation::Horizontal,10);
    header.add_css_class("studio-header");
    let hamburger=button("☰",false);
    let w=window.clone();let state2=state.clone();
    hamburger.connect_clicked(move |_| {
        let old=state2.borrow().sidebar_visible;
        state2.borrow_mut().sidebar_visible=!old;
        rebuild(&w,&state2);
    });
    header.append(&hamburger);
    let title=gtk::Label::new(Some("GRUB  Studio"));
    title.add_css_class("brand");title.set_xalign(0.0);title.set_hexpand(true);
    header.append(&title);
    let badge=gtk::Label::new(Some(if s.demo {"DEMO"} else if s.busy {"WORKING"} else if s.authorized&&s.snapshot.profile.writable {"GRUB ✓"}else{"READ ONLY"}));
    badge.add_css_class("status-badge");header.append(&badge);
    let appearance=button(if s.appearance=="light" {"☾"}else{"☼"},false);
    appearance.set_tooltip_text(Some(&tr(state,"theme.label")));
    let w=window.clone();let state2=state.clone();
    appearance.connect_clicked(move |_| {
        let mut s=state2.borrow_mut();
        s.appearance=if s.appearance=="light" {"dark"}else{"light"}.into();
        save_prefs(&s);drop(s);rebuild(&w,&state2);
    });
    header.append(&appearance);
    let names:Vec<&str>=LANGUAGES.iter().map(|(_,name)|*name).collect();
    let language=gtk::DropDown::from_strings(&names);
    language.add_css_class("lang-select");
    language.set_selected(LANGUAGES.iter().position(|(id,_)| *id==s.language).unwrap_or(0) as u32);
    let w=window.clone();let state2=state.clone();
    language.connect_selected_notify(move |combo| {
        if let Some((locale,_))=LANGUAGES.get(combo.selected() as usize) {
            let mut st=state2.borrow_mut();
            if st.language==*locale {return;}
            st.language=(*locale).into();save_prefs(&st);
            drop(st);rebuild(&w,&state2);
        }
    });
    header.append(&language);
    root.append(&header);

    let body=gtk::Box::new(gtk::Orientation::Horizontal,0);
    body.set_hexpand(true);body.set_vexpand(true);
    let stack=gtk::Stack::new();stack.set_hexpand(true);stack.set_vexpand(true);
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    let nav=gtk::Box::new(gtk::Orientation::Vertical,3);
    nav.add_css_class("studio-sidebar");
    nav.set_size_request(164,-1);
    add_text(&nav,&tr(state,"navigation"),"nav-heading",true);
    for (page,icon,key) in [
        ("home","▦","nav.home"),("menu","≡","nav.menu"),("systems","◫","nav.systems"),
        ("themes","◈","nav.themes"),("settings","⚙","nav.settings"),
        ("backups","↺","nav.backups"),("logs","≡","nav.logs"),("about","ⓘ","nav.about")] {
        let b=gtk::Button::with_label(&format!("{icon}   {}",tr(state,key)));
        b.add_css_class("nav-button");
        if s.active==page {b.add_css_class("nav-active");}
        b.set_halign(gtk::Align::Fill);b.set_hexpand(true);
        let w=window.clone();let st=state.clone();
        b.connect_clicked(move |_| {st.borrow_mut().active=page.into();rebuild(&w,&st);});
        nav.append(&b);
    }
    let spacer=gtk::Box::new(gtk::Orientation::Vertical,0);spacer.set_vexpand(true);
    nav.append(&spacer);
    add_text(&nav,&format!("v2.0.0 · {}",s.snapshot.profile.family.label()),"nav-footer",true);
    if s.sidebar_visible {body.append(&nav);}
    for (name,page) in [
        ("home",page_home(window,state)),("menu",page_menu(window,state)),
        ("systems",page_systems(window,state)),("themes",page_themes(window,state)),
        ("settings",page_settings(window,state)),("backups",page_backups(window,state)),
        ("logs",page_logs(window,state)),("about",page_about(window,state)),
    ]{
        let scroller=gtk::ScrolledWindow::new();
        scroller.set_policy(gtk::PolicyType::Never,gtk::PolicyType::Automatic);
        scroller.set_hexpand(true);scroller.set_vexpand(true);
        scroller.set_child(Some(&page));
        stack.add_named(&scroller,Some(name));
    }
    stack.set_visible_child_name(&s.active);
    body.append(&stack);
    root.append(&body);
    let footer=gtk::Box::new(gtk::Orientation::Horizontal,8);
    footer.add_css_class("studio-footer");
    let label=gtk::Label::new(Some(&tr(state,"footer")));
    label.set_hexpand(true);label.set_xalign(0.0);label.set_wrap(true);
    footer.append(&label);
    footer.append(&gtk::Label::new(Some(if s.demo {"DEMO"}else{"Wayland / GTK4"})));
    root.append(&footer);
    window.set_child(Some(&root));
}

fn main() {
    let args:Vec<String>=std::env::args().skip(1).collect();
    if args.iter().any(|a| a=="--version") { println!("GRUB Studio Universal 2.0.0 Rust Edition");return; }
    if args.iter().any(|a| a=="--self-test") {
        let _=serde_json::from_str::<serde_json::Value>(include_str!("../assets/translations.json")).expect("valid locale catalog");
        println!("Built-in translations: OK; run cargo test for full core checks");
        return;
    }
    unsafe extern "C" { fn geteuid() -> u32; }
    if unsafe { geteuid() } == 0 {
        eprintln!("Start the GTK window as a normal user, not with sudo. The helper uses Polkit.");
        std::process::exit(2);
    }
    let demo=args.iter().any(|a| a=="--demo");
    let app=gtk::Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| load_css());
    app.connect_activate(move |app| {
        let state=Rc::new(RefCell::new(initial_state(demo)));
        let window=gtk::ApplicationWindow::builder()
            .application(app).title("GRUB Studio Universal · Rust Edition")
            .default_width(960).default_height(625).build();
        rebuild(&window,&state);
        window.present();
    });
    let _=app.run();
}
