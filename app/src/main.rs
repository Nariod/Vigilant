//! GTK4/libadwaita UI for Vigilant.
//!
//! The entire UI state lives in a `NoteStore` held in memory; closing the
//! window destroys the process and every plaintext with it.

use gtk::{glib, prelude::*};
use libadwaita as adw;
use vigilant_core::NoteStore;
use std::cell::RefCell;
use std::rc::Rc;

const APP_ID: &str = "io.github.nariod.Vigilant";

struct App {
    store: RefCell<NoteStore>,
    notes_box: gtk::ListBox,
    empty_label: gtk::Label,
    stack: gtk::Stack,
    next_id: std::cell::Cell<u64>,
}

fn main() -> glib::ExitCode {
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .build();
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

    let notes_box = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(vec!["boxed-list".to_string()])
        .build();

    let empty_label = gtk::Label::new(Some(
        "Aucune note.\nLes notes disparaissent à la fermeture de l'application.",
    ));
    empty_label.set_valign(gtk::Align::Start);
    empty_label.set_margin_top(48);
    empty_label.set_justify(gtk::Justification::Center);
    empty_label.add_css_class("dim-label");

    let stack = gtk::Stack::new();
    stack.add_child(&empty_label);
    stack.add_child(&notes_box);
    stack.set_visible_child(&empty_label);

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    scrolled.set_child(Some(&stack));

    let header = adw::HeaderBar::new();
    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 12);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);
    vbox.append(&input_group);
    vbox.append(&scrolled);

    let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
    layout.append(&header);
    layout.append(&vbox);
    window.set_content(Some(&layout));

    let state = Rc::new(App {
        store: RefCell::new(NoteStore::new()),
        notes_box,
        empty_label,
        stack,
        next_id: std::cell::Cell::new(0),
    });

    {
        let state = Rc::clone(&state);
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

    refresh(&state);
    window.present();
}

fn refresh(state: &Rc<App>) {
    while let Some(row) = state.notes_box.first_child() {
        state.notes_box.remove(&row);
    }
    let ids = state.store.borrow().ids();
    if ids.is_empty() {
        state.stack.set_visible_child(&state.empty_label);
        return;
    }
    for id in ids {
        let content = state
            .store
            .borrow()
            .get(&id)
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
        {
            let state = Rc::clone(state);
            let id = id.clone();
            row.connect_activate(move |_row| {
                let _ = state.store.borrow_mut().delete(&id);
                refresh(&state);
            });
        }
        state.notes_box.append(&row);
    }
    state.stack.set_visible_child(&state.notes_box);
}
