use gtk::{
    glib::{self, ControlFlow},
    prelude::*,
};
use libadwaita::{self as adw, prelude::*};
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::Instant;
use zeroize::Zeroizing;

const APP_ID: &str = "io.github.nariod.Vigilant";
const CORE_LIMIT_ZERO: libc::rlim_t = 0;
const DEFAULT_WIPE_MINUTES: u32 = 10;
const MAX_WIPE_MINUTES: u32 = 480;
const MASKED_LABEL: &str = "••••••••";
const TICK_SECONDS: u32 = 60;

struct App {
    store: RefCell<vigilant_core::NoteStore>,
    window: RefCell<Option<adw::ApplicationWindow>>,
    search_entry: gtk::SearchEntry,
    input_buffer: RefCell<Option<gtk::TextBuffer>>,
    notes_box: gtk::ListBox,
    empty_label: gtk::Label,
    stack: gtk::Stack,
    banner: adw::Banner,
    next_id: std::cell::Cell<u64>,
    auto_wipe_source: RefCell<Option<glib::SourceId>>,
    auto_wipe_enabled: std::cell::Cell<bool>,
    auto_wipe_minutes: std::cell::Cell<u32>,
    last_activity: RefCell<Instant>,
}

fn main() -> glib::ExitCode {
    disable_core_dumps();
    set_non_dumpable();
    let memory_locked = lock_memory();
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| build_ui(app, memory_locked));
    app.run()
}

fn disable_core_dumps() {
    let limit = libc::rlimit {
        rlim_cur: CORE_LIMIT_ZERO,
        rlim_max: CORE_LIMIT_ZERO,
    };
    if unsafe { libc::setrlimit(libc::RLIMIT_CORE, &limit) } != 0 {
        eprintln!("warning: could not disable core dumps");
    }
}

fn set_non_dumpable() {
    if unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0) } != 0 {
        eprintln!("warning: could not set PR_SET_DUMPABLE");
    }
}

fn lock_memory() -> bool {
    raise_memlock_limit();
    let locked = mlockall_with(libc::MCL_CURRENT | libc::MCL_FUTURE | libc::MCL_ONFAULT)
        || mlockall_with(libc::MCL_CURRENT | libc::MCL_FUTURE);
    if !locked {
        eprintln!(
            "warning: could not lock memory (errno {}, memlock soft limit {} bytes); sensitive pages may be swapped",
            std::io::Error::last_os_error().raw_os_error().unwrap_or(0),
            memlock_soft_limit()
        );
    }
    locked
}

fn memlock_soft_limit() -> libc::rlim_t {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::getrlimit(libc::RLIMIT_MEMLOCK, &mut limit) } != 0 {
        return 0;
    }
    limit.rlim_cur
}

fn mlockall_with(flags: libc::c_int) -> bool {
    unsafe { libc::mlockall(flags) == 0 }
}

fn raise_memlock_limit() {
    let mut limit = libc::rlimit {
        rlim_cur: libc::RLIM_INFINITY,
        rlim_max: libc::RLIM_INFINITY,
    };
    if set_memlock(&limit) {
        return;
    }
    if unsafe { libc::getrlimit(libc::RLIMIT_MEMLOCK, &mut limit) } != 0 {
        eprintln!("warning: could not read RLIMIT_MEMLOCK");
        return;
    }
    if limit.rlim_max == libc::RLIM_INFINITY {
        limit.rlim_cur = libc::RLIM_INFINITY;
        if !set_memlock(&limit) {
            eprintln!("warning: could not raise RLIMIT_MEMLOCK");
        }
    }
}

fn set_memlock(limit: &libc::rlimit) -> bool {
    unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, limit) == 0 }
}

fn private_input_hints() -> gtk::InputHints {
    gtk::InputHints::PRIVATE | gtk::InputHints::NO_SPELLCHECK
}

fn build_ui(app: &adw::Application, memory_locked: bool) {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .default_width(480)
        .default_height(600)
        .title("Vigilant")
        .build();

    let state = Rc::new(App {
        store: RefCell::new(vigilant_core::NoteStore::new()),
        window: RefCell::new(None),
        search_entry: gtk::SearchEntry::new(),
        input_buffer: RefCell::new(None),
        notes_box: gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(vec!["boxed-list".to_string()])
            .build(),
        empty_label: build_empty_label(),
        stack: gtk::Stack::new(),
        banner: adw::Banner::new(""),
        next_id: std::cell::Cell::new(0),
        auto_wipe_source: RefCell::new(None),
        auto_wipe_enabled: std::cell::Cell::new(false),
        auto_wipe_minutes: std::cell::Cell::new(DEFAULT_WIPE_MINUTES),
        last_activity: RefCell::new(Instant::now()),
    });
    *state.window.borrow_mut() = Some(window.clone());

    state.search_entry.set_input_hints(private_input_hints());

    let input = gtk::TextView::builder()
        .wrap_mode(gtk::WrapMode::WordChar)
        .top_margin(8)
        .bottom_margin(8)
        .hexpand(true)
        .height_request(96)
        .build();
    input.set_input_hints(private_input_hints());
    input.set_input_purpose(gtk::InputPurpose::Password);
    input.buffer().set_enable_undo(false);

    let save_btn = gtk::Button::with_label("Add");
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
    layout.append(&state.banner);
    layout.append(&vbox);
    window.set_content(Some(&layout));

    if !memory_locked {
        state.banner.set_title(
            "Memory could not be locked: sensitive pages may reach the swap. See README.",
        );
        state.banner.set_revealed(true);
    }

    build_settings_popover(&state, &settings_btn);
    connect_save(&state, &save_btn, input.clone());
    *state.input_buffer.borrow_mut() = Some(input.buffer());
    connect_search(&state);
    connect_close(&state, &window);

    refresh(&state);
    window.present();
}

fn build_empty_label() -> gtk::Label {
    let label = gtk::Label::new(Some(
        "No notes yet.\nNotes disappear when the application closes.",
    ));
    label.set_valign(gtk::Align::Start);
    label.set_margin_top(48);
    label.set_justify(gtk::Justification::Center);
    label.add_css_class("dim-label");
    label
}

fn build_settings_popover(state: &Rc<App>, settings_btn: &gtk::MenuButton) {
    let popover = gtk::Popover::new();
    settings_btn.set_popover(Some(&popover));

    let wipe_switch = gtk::Switch::new();
    wipe_switch.set_valign(gtk::Align::Center);

    let wipe_row = adw::ActionRow::builder()
        .title("Automatic note wiping")
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
    timer_row.set_title("Idle delay (minutes)");

    let wipe_now_btn = gtk::Button::with_label("Erase");
    wipe_now_btn.add_css_class("destructive-action");
    wipe_now_btn.set_valign(gtk::Align::Center);

    let wipe_now_row = adw::ActionRow::builder()
        .title("Erase all notes now")
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

    connect_wipe_switch(state, &wipe_switch);
    connect_timer_row(state, &timer_row);
    connect_wipe_now(state, &wipe_now_btn);
}

fn connect_wipe_switch(state: &Rc<App>, wipe_switch: &gtk::Switch) {
    let weak: Weak<App> = Rc::downgrade(state);
    wipe_switch.connect_state_notify(move |switch| {
        if let Some(state) = weak.upgrade() {
            state.auto_wipe_enabled.set(switch.is_active());
            schedule_auto_wipe(&state);
        }
    });
}

fn connect_timer_row(state: &Rc<App>, timer_row: &adw::SpinRow) {
    let weak: Weak<App> = Rc::downgrade(state);
    timer_row.connect_changed(move |row| {
        if let Some(state) = weak.upgrade() {
            state.auto_wipe_minutes.set(row.value() as u32);
            schedule_auto_wipe(&state);
        }
    });
}

fn connect_wipe_now(state: &Rc<App>, wipe_now_btn: &gtk::Button) {
    let weak: Weak<App> = Rc::downgrade(state);
    wipe_now_btn.connect_clicked(move |_btn| {
        if let Some(state) = weak.upgrade() {
            wipe_everything(&state);
            refresh(&state);
        }
    });
}

fn schedule_auto_wipe(state: &Rc<App>) {
    cancel_auto_wipe(state);
    if !state.auto_wipe_enabled.get() {
        return;
    }
    let weak: Weak<App> = Rc::downgrade(state);
    let source = glib::timeout_add_seconds_local(TICK_SECONDS, move || match weak.upgrade() {
        Some(state) => {
            auto_wipe_tick(&state);
            ControlFlow::Continue
        }
        None => ControlFlow::Break,
    });
    state.auto_wipe_source.borrow_mut().replace(source);
}

fn auto_wipe_tick(state: &Rc<App>) {
    let threshold = state.auto_wipe_minutes.get().clamp(1, MAX_WIPE_MINUTES) as u64;
    let idle_secs = state.last_activity.borrow().elapsed().as_secs();
    if idle_secs < threshold.saturating_mul(60) {
        return;
    }
    wipe_everything(state);
    refresh(state);
}

fn cancel_auto_wipe(state: &Rc<App>) {
    if let Some(source) = state.auto_wipe_source.borrow_mut().take() {
        source.remove();
    }
}

fn wipe_everything(state: &Rc<App>) {
    state.store.borrow_mut().clear();
    state.search_entry.set_text("");
    if let Some(buffer) = state.input_buffer.borrow().as_ref() {
        buffer.set_text("");
    }
    *state.last_activity.borrow_mut() = Instant::now();
}

fn full_shutdown_wipe(state: &Rc<App>) {
    wipe_everything(state);
    clear_clipboards();
}

fn clear_clipboards() {
    if let Some(display) = gtk::gdk::Display::default() {
        display.clipboard().set_text("");
        display.primary_clipboard().set_text("");
    }
}

fn connect_close(state: &Rc<App>, window: &adw::ApplicationWindow) {
    let weak: Weak<App> = Rc::downgrade(state);
    window.connect_close_request(move |_| {
        if let Some(state) = weak.upgrade() {
            full_shutdown_wipe(&state);
        }
        glib::Propagation::Proceed
    });
}

fn connect_save(state: &Rc<App>, save_btn: &gtk::Button, input: gtk::TextView) {
    let weak: Weak<App> = Rc::downgrade(state);
    save_btn.connect_clicked(move |_btn| {
        let Some(state) = weak.upgrade() else { return };
        let buffer = input.buffer();
        let (mut start, mut end) = (buffer.start_iter(), buffer.end_iter());
        let text = Zeroizing::new(buffer.text(&start, &end, false).to_string());
        if text.trim().is_empty() {
            return;
        }
        let id = format!("n-{}", state.next_id.get());
        state.next_id.set(state.next_id.get().saturating_add(1));
        if let Err(e) = state.store.borrow_mut().put(&id, &text) {
            eprintln!("warning: could not save note: {e}");
        }
        buffer.delete(&mut start, &mut end);
        *state.last_activity.borrow_mut() = Instant::now();
        refresh(&state);
    });
}

fn connect_search(state: &Rc<App>) {
    let weak: Weak<App> = Rc::downgrade(state);
    state.search_entry.connect_search_changed(move |_| {
        if let Some(state) = weak.upgrade() {
            *state.last_activity.borrow_mut() = Instant::now();
            refresh(&state);
        }
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
    let label = gtk::Label::new(None);
    label.set_wrap(true);
    label.set_xalign(0.0);
    label.set_margin_top(8);
    label.set_margin_bottom(8);
    label.set_margin_start(12);
    label.set_margin_end(12);
    label.set_text(MASKED_LABEL);

    let reveal_btn = gtk::Button::new();
    reveal_btn.set_icon_name("eye-open-symbolic");
    reveal_btn.set_valign(gtk::Align::Center);
    reveal_btn.add_css_class("flat");

    let delete_btn = gtk::Button::new();
    delete_btn.set_icon_name("user-trash-symbolic");
    delete_btn.set_valign(gtk::Align::Center);
    delete_btn.add_css_class("flat");

    let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    label.set_hexpand(true);
    row_box.append(&label);
    row_box.append(&reveal_btn);
    row_box.append(&delete_btn);

    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&row_box));
    connect_reveal(state.clone(), &reveal_btn, &label, id);
    connect_delete(state.clone(), &delete_btn, id);
    state.notes_box.append(&row);
}

fn connect_reveal(state: Rc<App>, reveal_btn: &gtk::Button, label: &gtk::Label, id: &str) {
    let weak: Weak<App> = Rc::downgrade(&state);
    let label = label.clone();
    let id = id.to_string();
    let revealed = std::cell::Cell::new(false);
    reveal_btn.connect_clicked(move |btn| {
        let Some(state) = weak.upgrade() else { return };
        *state.last_activity.borrow_mut() = Instant::now();
        if revealed.get() {
            label.set_text(MASKED_LABEL);
            btn.set_icon_name("eye-open-symbolic");
            revealed.set(false);
        } else {
            let display = state
                .store
                .borrow()
                .get(&id)
                .map(|s| s.to_string())
                .unwrap_or_else(|_| "<unreadable note>".to_string());
            label.set_text(&display);
            btn.set_icon_name("eye-shut-symbolic");
            revealed.set(true);
        }
    });
}

fn connect_delete(state: Rc<App>, delete_btn: &gtk::Button, id: &str) {
    let weak: Weak<App> = Rc::downgrade(&state);
    let id = id.to_string();
    delete_btn.connect_clicked(move |_btn| {
        let Some(state) = weak.upgrade() else { return };
        let dialog = gtk::AlertDialog::builder()
            .message("Delete this note?")
            .detail("This cannot be undone.")
            .buttons(vec!["Cancel".to_string(), "Delete".to_string()])
            .default_button(1)
            .cancel_button(0)
            .modal(true)
            .build();
        let weak = weak.clone();
        let id = id.clone();
        let parent = state.window.borrow().clone();
        dialog.choose(
            parent.as_ref(),
            gtk::gio::Cancellable::NONE,
            move |response| {
                if response == Ok(1) {
                    if let Some(state) = weak.upgrade() {
                        state.store.borrow_mut().delete(&id);
                        refresh(&state);
                    }
                }
            },
        );
    });
}
