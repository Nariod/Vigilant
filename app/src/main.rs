use gtk::{
    glib::{self, ControlFlow},
    prelude::*,
};
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

const APP_ID: &str = "io.github.nariod.Vigilant";
const AUTO_LOCK_MINUTES: u64 = 5;
const AUTO_LOCK_TICK_MS: u32 = 10_000;

struct App {
    store: RefCell<vigilant_core::NoteStore>,
    search_entry: gtk::SearchEntry,
    notes_box: gtk::ListBox,
    empty_label: gtk::Label,
    stack: gtk::Stack,
    next_id: std::cell::Cell<u64>,
    is_locked: std::cell::Cell<bool>,
}

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
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
        is_locked: std::cell::Cell::new(false),
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

    let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
    layout.append(&adw::HeaderBar::new());
    layout.append(&vbox);
    window.set_content(Some(&layout));

    connect_save(state.clone(), &save_btn, &input);
    connect_search(state.clone());
    start_auto_lock(state.clone(), &window);

    refresh(&state);
    window.present();
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
        let buffer = input.buffer();
        let (start, end) = (buffer.start_iter(), buffer.end_iter());
        let mut text = buffer.text(&start, &end, false).to_string();
        if text.trim().is_empty() {
            return;
        }
        let id = format!("n-{}", state.next_id.get());
        state.next_id.set(state.next_id.get() + 1);
        let _ = state.store.borrow_mut().put(&id, &text);
        zeroize::Zeroize::zeroize(&mut text);
        buffer.delete(&start, &end);
        refresh(&state);
    });
}

fn connect_search(state: Rc<App>) {
    state.search_entry.connect_search_changed(move |entry| {
        refresh(&state);
        let _ = entry;
    });
}

fn start_auto_lock(state: Rc<App>, window: &adw::ApplicationWindow) {
    window.set_focusable(true);
    let last_activity = Rc::new(std::cell::Cell::new(std::time::Instant::now()));
    track_activity(last_activity.clone(), window);
    glib::timeout_add_seconds_local(AUTO_LOCK_MINUTES, move || {
        if last_activity.get().elapsed() >= Duration::from_secs(AUTO_LOCK_MINUTES) {
            state.store.borrow_mut().clear();
            state.is_locked.set(true);
            refresh(&state);
            show_locked_overlay(&state, window);
            return ControlFlow::Break;
        }
        ControlFlow::Continue
    });
}

fn track_activity(last_activity: Rc<std::cell::Cell<std::time::Instant>>, window: &adw::ApplicationWindow) {
    for signal in ["notify::has-focus", "key-press-event"] {
        window.connect_local(signal, false, move |_args| {
            last_activity.set(std::time::Instant::now());
            None
        });
    }
}

fn show_locked_overlay(state: &Rc<App>, window: &adw::ApplicationWindow) {
    let dialog = adw::AlertDialog::new(
        Some("Verrouillé"),
        Some("Toutes les notes ont été effacées après inactivité."),
    );
    dialog.add_response("ok", "Déverrouiller");
    let state = state.clone();
    dialog.choose(window, None::<&gtk::Cancellable>, move |_response| {
        state.is_locked.set(false);
    });
}

fn current_query(state: &Rc<App>) -> String {
    state.search_entry.text().to_string()
}

fn refresh(state: &Rc<App>) {
    clear_rows(state);
    let query = current_query(state);
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
    let state = state;
    let id = id.to_string();
    row.connect_activate(move |_row| {
        let _ = state.store.borrow_mut().delete(&id);
        refresh(&state);
    });
}
