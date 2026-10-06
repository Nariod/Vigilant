use gtk::{
    glib::{self, ControlFlow},
    prelude::*,
};
use libadwaita::{self as adw, prelude::*};
use std::cell::RefCell;
use std::rc::Rc;

const APP_ID: &str = "io.github.nariod.Vigilant";
const CORE_LIMIT_ZERO: libc::rlim_t = 0;
const DEFAULT_WIPE_MINUTES: u32 = 10;
const MAX_WIPE_MINUTES: u32 = 480;

struct App {
    store: RefCell<vigilant_core::NoteStore>,
    search_entry: gtk::SearchEntry,
    notes_box: gtk::ListBox,
    empty_label: gtk::Label,
    stack: gtk::Stack,
    next_id: std::cell::Cell<u64>,
    auto_wipe_source: RefCell<Option<glib::SourceId>>,
    auto_wipe_enabled: std::cell::Cell<bool>,
    auto_wipe_minutes: std::cell::Cell<u32>,
}

fn main() -> glib::ExitCode {
    disable_core_dumps();
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn disable_core_dumps() {
    let limit = libc::rlimit {
        rlim_cur: CORE_LIMIT_ZERO,
        rlim_max: CORE_LIMIT_ZERO,
    };
    unsafe { libc::setrlimit(libc::RLIMIT_CORE, &limit) };
}

fn build_ui(app: &adw::Application) {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .default_width(480)
        .default_height(600)
        .title("Vigilant")
        .build();

    let state = Rc::new(App {
        store: RefCell::new(vigilant_core::NoteStore::new()),
        search_entry: gtk::SearchEntry::new(),
        notes_box: gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(vec!["boxed-list".to_string()])
            .build(),
        empty_label: build_empty_label(),
        stack: gtk::Stack::new(),
        next_id: std::cell::Cell::new(0),
        auto_wipe_source: RefCell::new(None),
        auto_wipe_enabled: std::cell::Cell::new(false),
        auto_wipe_minutes: std::cell::Cell::new(DEFAULT_WIPE_MINUTES),
    });

    let input = gtk::TextView::builder()
        .wrap_mode(gtk::WrapMode::WordChar)
        .top_margin(8)
        .bottom_margin(8)
        .hexpand(true)
        .height_request(96)
        .build();

    let save_btn = gtk::Button::with_label("Ajouter");
    save_btn.add_css_class("suggested-action");
    save_btn.set_halign(gtk::Align::End);

    let input_group = adw::PreferencesGroup::new();
    input_group.add(&input);
    input_group.add(&save_btn);

    state.stack.add_child(&state.empty_label);
    state.stack.add_child(&state.notes_box);
    state.stack.set_visible_child(&state.empty_label);

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    scrolled.set_child(Some(&state.stack));

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 12);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);
    vbox.append(&state.search_entry);
    vbox.append(&input_group);
    vbox.append(&scrolled);

    let header = adw::HeaderBar::new();
    let settings_btn = gtk::MenuButton::new();
    settings_btn.set_icon_name("settings-symbolic");
    header.pack_end(&settings_btn);

    let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
    layout.append(&header);
    layout.append(&vbox);
    window.set_content(Some(&layout));

    build_settings_popover(&state, &settings_btn);
    connect_save(state.clone(), &save_btn, &input);
    connect_search(state.clone());

    refresh(&state);
    window.present();
}

fn build_settings_popover(state: &Rc<App>, settings_btn: &gtk::MenuButton) {
    let popover = gtk::Popover::new();
    settings_btn.set_popover(Some(&popover));

    let wipe_switch = gtk::Switch::new();
    wipe_switch.set_valign(gtk::Align::Center);

    let wipe_row = adw::ActionRow::builder()
        .title("Effacement automatique des notes")
        .build();
    wipe_row.add_suffix(&wipe_switch);

    let adjustment = gtk::Adjustment::new(
        DEFAULT_WIPE_MINUTES as f64,
        1.0,
        MAX_WIPE_MINUTES as f64,
        1.0,
        10.0,
        0.0,
    );
    let timer_row = adw::SpinRow::new(Some(&adjustment), 1.0, 0);
    timer_row.set_title("Délai (minutes)");

    let wipe_now_btn = gtk::Button::with_label("Effacer");
    wipe_now_btn.add_css_class("destructive-action");
    wipe_now_btn.set_valign(gtk::Align::Center);

    let wipe_now_row = adw::ActionRow::builder()
        .title("Effacer toutes les notes maintenant")
        .build();
    wipe_now_row.add_suffix(&wipe_now_btn);

    let prefs_group = adw::PreferencesGroup::new();
    prefs_group.add(&wipe_row);
    prefs_group.add(&timer_row);
    prefs_group.add(&wipe_now_row);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.append(&prefs_group);
    popover.set_child(Some(&content));

    connect_wipe_switch(Rc::clone(state), &wipe_switch);
    connect_timer_row(Rc::clone(state), &timer_row);
    connect_wipe_now(Rc::clone(state), &wipe_now_btn);
}

fn connect_wipe_switch(state: Rc<App>, wipe_switch: &gtk::Switch) {
    wipe_switch.connect_state_notify(move |switch| {
        state.auto_wipe_enabled.set(switch.is_active());
        schedule_auto_wipe(&state);
    });
}

fn connect_timer_row(state: Rc<App>, timer_row: &adw::SpinRow) {
    timer_row.connect_changed(move |row| {
        state.auto_wipe_minutes.set(row.value() as u32);
        schedule_auto_wipe(&state);
    });
}

fn connect_wipe_now(state: Rc<App>, wipe_now_btn: &gtk::Button) {
    wipe_now_btn.connect_clicked(move |_btn| {
        state.store.borrow_mut().clear();
        refresh(&state);
    });
}

fn schedule_auto_wipe(state: &Rc<App>) {
    cancel_auto_wipe(state);
    if !state.auto_wipe_enabled.get() {
        return;
    }
    let seconds = state.auto_wipe_minutes.get().max(1) * 60;
    let timer_state = Rc::clone(state);
    let source = glib::timeout_add_seconds_local(seconds, move || {
        timer_state.store.borrow_mut().clear();
        refresh(&timer_state);
        ControlFlow::Continue
    });
    state.auto_wipe_source.borrow_mut().replace(source);
}

fn cancel_auto_wipe(state: &Rc<App>) {
    if let Some(source) = state.auto_wipe_source.borrow_mut().take() {
        source.remove();
    }
}

fn build_empty_label() -> gtk::Label {
    let label = gtk::Label::new(Some(
        "Aucune note.\nLes notes disparaissent à la fermeture de l'application.",
    ));
    label.set_valign(gtk::Align::Start);
    label.set_margin_top(48);
    label.set_justify(gtk::Justification::Center);
    label.add_css_class("dim-label");
    label
}

fn connect_save(state: Rc<App>, save_btn: &gtk::Button, input: &gtk::TextView) {
    save_btn.connect_clicked(move |_btn| {
        let mut buffer = input.buffer();
        let (mut start, mut end) = (buffer.start_iter(), buffer.end_iter());
        let mut text = buffer.text(&start, &end, false).to_string();
        if text.trim().is_empty() {
            return;
        }
        let id = format!("n-{}", state.next_id.get());
        state.next_id.set(state.next_id.get() + 1);
        let _ = state.store.borrow_mut().put(&id, &text);
        zeroize::Zeroize::zeroize(&mut text);
        buffer.delete(&mut start, &mut end);
        refresh(&state);
    });
}

fn connect_search(state: Rc<App>) {
    let search_state = Rc::clone(&state);
    state.search_entry.connect_search_changed(move |_| {
        refresh(&search_state);
    });
}

fn refresh(state: &Rc<App>) {
    clear_rows(state);
    let query = state.search_entry.text().to_string();
    let ids = state.store.borrow().search(&query);
    if ids.is_empty() {
        state.stack.set_visible_child(&state.empty_label);
        return;
    }
    for id in ids {
        append_note_row(state, &id);
    }
    state.stack.set_visible_child(&state.notes_box);
}

fn clear_rows(state: &Rc<App>) {
    while let Some(row) = state.notes_box.first_child() {
        state.notes_box.remove(&row);
    }
}

fn append_note_row(state: &Rc<App>, id: &str) {
    let content = state
        .store
        .borrow()
        .get(id)
        .map(|s| s.to_string())
        .unwrap_or_else(|_| "<note illisible>".to_string());
    let label = gtk::Label::new(None);
    label.set_wrap(true);
    label.set_xalign(0.0);
    label.set_margin_top(8);
    label.set_margin_bottom(8);
    label.set_margin_start(12);
    label.set_margin_end(12);
    label.set_text(&content);
    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&label));
    connect_delete_on_click(state.clone(), &row, id);
    state.notes_box.append(&row);
}

fn connect_delete_on_click(state: Rc<App>, row: &gtk::ListBoxRow, id: &str) {
    let id = id.to_string();
    row.connect_activate(move |_row| {
        let _ = state.store.borrow_mut().delete(&id);
        refresh(&state);
    });
}
