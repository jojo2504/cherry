use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Adjustment, Application, ApplicationWindow, Box as GtkBox, Button, ComboBoxText, Grid, Label,
    Orientation, Scale, ScrolledWindow, SpinButton, Switch,
};
use std::cell::RefCell;
use std::rc::Rc;

const APP_ID: &str = "com.example.p2pscreenshare";

// Peer structure to hold peer information
#[derive(Clone, Debug)]
struct Peer {
    id: String,
    name: String,
    status: PeerStatus,
    latency: Option<u32>, // in ms
}

#[derive(Clone, Debug, PartialEq)]
enum PeerStatus {
    Online,
    Offline,
    Connected,
}

#[derive(Clone, Debug, PartialEq)]
enum StreamStatus {
    Idle,
    Streaming,
    Viewing,
}

// App state
struct AppState {
    peers: Vec<Peer>,
    stream_status: StreamStatus,
    volume: f64,
    muted: bool,
    selected_screen: Option<String>,
    resolution: String,
    fps: f64,
    quality: String,
    hw_accel: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            peers: Vec::new(),
            stream_status: StreamStatus::Idle,
            volume: 75.0,
            muted: false,
            selected_screen: None,
            resolution: "Auto".to_string(),
            fps: 30.0,
            quality: "High".to_string(),
            hw_accel: true,
        }
    }
}

pub async fn main() -> glib::ExitCode {
    gtk4::init().unwrap();

    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(build_ui);

    // Apply custom CSS for VS Code dark theme aesthetic
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(
        r#"
        window {
            background-color: #1e1e1e;
        }

        .titlebar {
            background: #252525;
            border-bottom: 1px solid #3c3c3c;
            min-height: 48px;
        }

        .titlebar-label {
            color: #cccccc;
            font-size: 13px;
            font-weight: 400;
        }

        .status-indicator {
            background-color: #007acc;
            color: #ffffff;
            border-radius: 2px;
            padding: 3px 8px;
            font-size: 11px;
            font-weight: 500;
        }

        .status-indicator.streaming {
            background-color: #f48771;
        }

        .status-indicator.idle {
            background-color: #3c3c3c;
        }

        .section-card {
            background-color: #252525;
            border: 1px solid #3c3c3c;
            border-radius: 6px;
            padding: 16px;
        }

        .section-header {
            color: #cccccc;
            font-size: 13px;
            font-weight: 600;
            margin-bottom: 12px;
        }

        .section-subheader {
            color: #858585;
            font-size: 11px;
            font-weight: 500;
            text-transform: uppercase;
            letter-spacing: 0.5px;
            margin-bottom: 8px;
        }

        button.primary {
            background-color: #0e639c;
            color: #ffffff;
            border: 1px solid #007acc;
            border-radius: 2px;
            padding: 8px 16px;
            font-size: 13px;
            font-weight: 500;
            min-height: 28px;
        }

        button.primary:hover {
            background-color: #1177bb;
        }

        button.primary.destructive {
            background-color: #a1260d;
            border-color: #f48771;
        }

        button.primary.destructive:hover {
            background-color: #c72e0d;
        }

        button.secondary {
            background-color: #2d2d2d;
            color: #cccccc;
            border: 1px solid #3c3c3c;
            border-radius: 2px;
            padding: 6px 12px;
            font-size: 12px;
            font-weight: 400;
            min-height: 26px;
        }

        button.secondary:hover {
            background-color: #3c3c3c;
        }

        button.icon-button {
            background-color: transparent;
            color: #cccccc;
            border: none;
            border-radius: 2px;
            padding: 4px 8px;
            min-width: 28px;
            min-height: 28px;
        }

        button.icon-button:hover {
            background-color: #2d2d2d;
        }

        .peer-row {
            background-color: #2d2d2d;
            border: 1px solid #3c3c3c;
            border-radius: 3px;
            padding: 12px;
            margin-bottom: 6px;
        }

        .peer-row:hover {
            background-color: #333333;
            border-color: #4c4c4c;
        }

        .peer-name {
            color: #cccccc;
            font-size: 13px;
            font-weight: 500;
        }

        .peer-status {
            color: #858585;
            font-size: 11px;
        }

        .status-dot {
            font-size: 10px;
            margin-right: 4px;
        }

        .status-online {
            color: #89d185;
        }

        .status-offline {
            color: #6e6e6e;
        }

        .status-connected {
            color: #4fc1ff;
        }

        combobox, entry, spinbutton {
            background-color: #3c3c3c;
            color: #cccccc;
            border: 1px solid #3c3c3c;
            border-radius: 2px;
            padding: 6px 8px;
            font-size: 13px;
        }

        combobox:focus, entry:focus, spinbutton:focus {
            border-color: #007acc;
        }

        scale trough {
            background-color: #3c3c3c;
            border-radius: 2px;
            min-height: 4px;
        }

        scale highlight {
            background-color: #007acc;
            border-radius: 2px;
        }

        scale slider {
            background-color: #cccccc;
            border: none;
            border-radius: 8px;
            min-width: 14px;
            min-height: 14px;
            margin: -5px;
        }

        switch {
            background-color: #3c3c3c;
            border: 1px solid #3c3c3c;
        }

        switch:checked {
            background-color: #007acc;
        }

        switch slider {
            background-color: #cccccc;
            border-radius: 8px;
        }

        .empty-state {
            color: #6e6e6e;
            font-size: 12px;
            padding: 40px 20px;
        }

        .info-label {
            color: #cccccc;
            font-size: 12px;
        }

        .dim-label {
            color: #858585;
            font-size: 11px;
        }

        scrolledwindow {
            background-color: transparent;
        }

        list {
            background-color: transparent;
            border: none;
        }

        row {
            background-color: transparent;
        }

        separator {
            background-color: #3c3c3c;
            min-height: 1px;
        }
        "#,
    );

    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().expect("Could not connect to display"),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    app.run()
}

fn build_ui(app: &Application) {
    let state = Rc::new(RefCell::new(AppState::default()));

    // Main window
    let window = ApplicationWindow::builder()
        .application(app)
        .title("P2P Screen Share")
        .default_width(1000)
        .default_height(700)
        .build();

    // Main vertical container
    let main_box = GtkBox::new(Orientation::Vertical, 0);

    // Title bar
    let titlebar = create_titlebar(state.clone());
    main_box.append(&titlebar);

    // Content area
    let content_box = GtkBox::new(Orientation::Horizontal, 0);
    content_box.set_margin_top(0);
    content_box.set_margin_bottom(16);
    content_box.set_margin_start(16);
    content_box.set_margin_end(16);

    // Left panel - Main controls
    let left_panel = create_left_panel(state.clone());
    content_box.append(&left_panel);

    // Right panel - Peers
    let right_panel = create_right_panel(state.clone());
    content_box.append(&right_panel);

    main_box.append(&content_box);

    window.set_child(Some(&main_box));
    window.present();
}

fn create_titlebar(state: Rc<RefCell<AppState>>) -> GtkBox {
    let titlebar = GtkBox::new(Orientation::Horizontal, 12);
    titlebar.add_css_class("titlebar");
    titlebar.set_margin_start(16);
    titlebar.set_margin_end(16);
    titlebar.set_margin_top(12);
    titlebar.set_margin_bottom(12);

    let title = Label::new(Some("P2P Screen Share"));
    title.set_halign(gtk4::Align::Start);
    title.add_css_class("titlebar-label");
    titlebar.append(&title);

    titlebar.set_hexpand(true);

    // Status indicator
    let status_text = match state.borrow().stream_status {
        StreamStatus::Idle => "Idle",
        StreamStatus::Streaming => "Streaming",
        StreamStatus::Viewing => "Viewing",
    };

    let status = Label::new(Some(status_text));
    status.add_css_class("status-indicator");
    match state.borrow().stream_status {
        StreamStatus::Idle => status.add_css_class("idle"),
        StreamStatus::Streaming => status.add_css_class("streaming"),
        _ => {}
    }
    titlebar.append(&status);

    titlebar
}

fn create_left_panel(state: Rc<RefCell<AppState>>) -> GtkBox {
    let panel = GtkBox::new(Orientation::Vertical, 16);
    panel.set_size_request(480, -1);
    panel.set_margin_top(16);
    panel.set_margin_end(8);

    // Stream section
    let stream_section = create_stream_section(state.clone());
    panel.append(&stream_section);

    // Audio section
    let audio_section = create_audio_section(state.clone());
    panel.append(&audio_section);

    // Settings section
    let settings_section = create_settings_section(state.clone());
    panel.append(&settings_section);

    panel
}

fn create_right_panel(state: Rc<RefCell<AppState>>) -> GtkBox {
    let panel = GtkBox::new(Orientation::Vertical, 16);
    panel.set_size_request(480, -1);
    panel.set_margin_top(16);
    panel.set_margin_start(8);

    // Peers section
    let peers_section = create_peers_section(state.clone());
    panel.append(&peers_section);

    panel
}

fn create_stream_section(state: Rc<RefCell<AppState>>) -> GtkBox {
    let section = GtkBox::new(Orientation::Vertical, 12);
    section.add_css_class("section-card");

    let header = Label::new(Some("Stream"));
    header.set_halign(gtk4::Align::Start);
    header.add_css_class("section-header");
    section.append(&header);

    // Screen selection
    let screen_label = Label::new(Some("Display Source"));
    screen_label.set_halign(gtk4::Align::Start);
    screen_label.add_css_class("section-subheader");
    section.append(&screen_label);

    let screen_combo = ComboBoxText::new();
    screen_combo.append_text("Primary Monitor");
    screen_combo.append_text("Secondary Monitor");
    screen_combo.append_text("Select Window...");
    screen_combo.set_active(Some(0));

    let state_clone = state.clone();
    screen_combo.connect_changed(move |combo| {
        if let Some(text) = combo.active_text() {
            state_clone.borrow_mut().selected_screen = Some(text.to_string());
        }
    });

    section.append(&screen_combo);

    // Start/Stop button
    let button_box = GtkBox::new(Orientation::Horizontal, 8);
    button_box.set_margin_top(4);

    let stream_button = Button::with_label("Start Streaming");
    stream_button.add_css_class("primary");
    stream_button.set_hexpand(true);

    let state_clone = state.clone();
    stream_button.connect_clicked(move |button| {
        let mut state = state_clone.borrow_mut();
        match state.stream_status {
            StreamStatus::Idle | StreamStatus::Viewing => {
                state.stream_status = StreamStatus::Streaming;
                button.set_label("Stop Streaming");
                button.add_css_class("destructive");
            }
            StreamStatus::Streaming => {
                state.stream_status = StreamStatus::Idle;
                button.set_label("Start Streaming");
                button.remove_css_class("destructive");
            }
        }
    });

    button_box.append(&stream_button);
    section.append(&button_box);

    section
}

fn create_audio_section(state: Rc<RefCell<AppState>>) -> GtkBox {
    let section = GtkBox::new(Orientation::Vertical, 12);
    section.add_css_class("section-card");

    let header = Label::new(Some("Audio"));
    header.set_halign(gtk4::Align::Start);
    header.add_css_class("section-header");
    section.append(&header);

    // Mute toggle
    let mute_box = GtkBox::new(Orientation::Horizontal, 12);

    let mute_label = Label::new(Some("Mute Audio"));
    mute_label.set_halign(gtk4::Align::Start);
    mute_label.set_hexpand(true);
    mute_label.add_css_class("info-label");

    let mute_switch = Switch::new();
    mute_switch.set_active(false);

    let state_clone = state.clone();
    mute_switch.connect_state_set(move |_, enabled| {
        state_clone.borrow_mut().muted = enabled;
        glib::Propagation::Proceed
    });

    mute_box.append(&mute_label);
    mute_box.append(&mute_switch);
    section.append(&mute_box);

    // Volume slider
    let volume_label = Label::new(Some("Volume"));
    volume_label.set_halign(gtk4::Align::Start);
    volume_label.add_css_class("section-subheader");
    volume_label.set_margin_top(8);
    section.append(&volume_label);

    let volume_scale = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 1.0);
    volume_scale.set_value(state.borrow().volume);
    volume_scale.set_draw_value(true);
    volume_scale.set_value_pos(gtk4::PositionType::Right);

    let state_clone = state.clone();
    volume_scale.connect_value_changed(move |scale| {
        state_clone.borrow_mut().volume = scale.value();
    });

    section.append(&volume_scale);

    section
}

fn create_settings_section(state: Rc<RefCell<AppState>>) -> GtkBox {
    let section = GtkBox::new(Orientation::Vertical, 12);
    section.add_css_class("section-card");

    let header = Label::new(Some("Stream Settings"));
    header.set_halign(gtk4::Align::Start);
    header.add_css_class("section-header");
    section.append(&header);

    let grid = Grid::new();
    grid.set_row_spacing(12);
    grid.set_column_spacing(12);

    // Quality
    let quality_label = Label::new(Some("Quality Preset"));
    quality_label.set_halign(gtk4::Align::Start);
    quality_label.add_css_class("section-subheader");

    let quality_combo = ComboBoxText::new();
    quality_combo.append_text("High");
    quality_combo.append_text("Medium");
    quality_combo.append_text("Low");
    quality_combo.set_active(Some(0));

    let state_clone = state.clone();
    quality_combo.connect_changed(move |combo| {
        if let Some(text) = combo.active_text() {
            state_clone.borrow_mut().quality = text.to_string();
        }
    });

    grid.attach(&quality_label, 0, 0, 1, 1);
    grid.attach(&quality_combo, 0, 1, 1, 1);

    // FPS
    let fps_label = Label::new(Some("Framerate (FPS)"));
    fps_label.set_halign(gtk4::Align::Start);
    fps_label.add_css_class("section-subheader");

    let fps_adjustment = Adjustment::new(30.0, 15.0, 60.0, 5.0, 15.0, 0.0);
    let fps_spin = SpinButton::new(Some(&fps_adjustment), 1.0, 0);

    let state_clone = state.clone();
    fps_spin.connect_value_changed(move |spin| {
        state_clone.borrow_mut().fps = spin.value();
    });

    grid.attach(&fps_label, 1, 0, 1, 1);
    grid.attach(&fps_spin, 1, 1, 1, 1);

    // Hardware acceleration
    let hw_box = GtkBox::new(Orientation::Horizontal, 12);
    hw_box.set_margin_top(8);

    let hw_label = Label::new(Some("Hardware Acceleration"));
    hw_label.set_halign(gtk4::Align::Start);
    hw_label.set_hexpand(true);
    hw_label.add_css_class("info-label");

    let hw_switch = Switch::new();
    hw_switch.set_active(state.borrow().hw_accel);

    let state_clone = state.clone();
    hw_switch.connect_state_set(move |_, enabled| {
        state_clone.borrow_mut().hw_accel = enabled;
        glib::Propagation::Proceed
    });

    hw_box.append(&hw_label);
    hw_box.append(&hw_switch);

    grid.attach(&hw_box, 0, 2, 2, 1);

    section.append(&grid);
    section
}

fn create_peers_section(state: Rc<RefCell<AppState>>) -> GtkBox {
    let section = GtkBox::new(Orientation::Vertical, 12);
    section.add_css_class("section-card");
    section.set_vexpand(true);

    // Header with refresh button
    let header_box = GtkBox::new(Orientation::Horizontal, 12);

    let header = Label::new(Some("Discovered Peers"));
    header.set_halign(gtk4::Align::Start);
    header.set_hexpand(true);
    header.add_css_class("section-header");

    let refresh_button = Button::with_label("⟳");
    refresh_button.add_css_class("icon-button");
    refresh_button.set_tooltip_text(Some("Refresh peers"));

    refresh_button.connect_clicked(move |_| {
        println!("Refreshing peers...");
    });

    header_box.append(&header);
    header_box.append(&refresh_button);
    section.append(&header_box);

    // Peer list
    let peers = state.borrow().peers.clone();

    if peers.is_empty() {
        let empty = Label::new(Some(
            "No peers discovered\n\nMake sure you're connected to the same network",
        ));
        empty.add_css_class("empty-state");
        empty.set_justify(gtk4::Justification::Center);
        section.append(&empty);
    } else {
        let peer_list = GtkBox::new(Orientation::Vertical, 6);

        for peer in peers {
            let peer_item = create_peer_item(peer, state.clone());
            peer_list.append(&peer_item);
        }

        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .min_content_height(200)
            .child(&peer_list)
            .build();

        section.append(&scrolled);
    }

    section
}

fn create_peer_item(peer: Peer, _state: Rc<RefCell<AppState>>) -> GtkBox {
    let container = GtkBox::new(Orientation::Horizontal, 12);
    container.add_css_class("peer-row");

    // Info section
    let info_box = GtkBox::new(Orientation::Vertical, 4);
    info_box.set_hexpand(true);

    let name = Label::new(Some(&peer.name));
    name.set_halign(gtk4::Align::Start);
    name.add_css_class("peer-name");

    let status_box = GtkBox::new(Orientation::Horizontal, 4);

    let status_dot = Label::new(Some("●"));
    match peer.status {
        PeerStatus::Online => status_dot.add_css_class("status-online"),
        PeerStatus::Offline => status_dot.add_css_class("status-offline"),
        PeerStatus::Connected => status_dot.add_css_class("status-connected"),
    }
    status_dot.add_css_class("status-dot");

    let status_text = match peer.status {
        PeerStatus::Online => {
            if let Some(latency) = peer.latency {
                format!("Online · {}ms", latency)
            } else {
                "Online".to_string()
            }
        }
        PeerStatus::Offline => "Offline".to_string(),
        PeerStatus::Connected => "Connected".to_string(),
    };
    let status_label = Label::new(Some(&status_text));
    status_label.add_css_class("peer-status");

    status_box.append(&status_dot);
    status_box.append(&status_label);

    info_box.append(&name);
    info_box.append(&status_box);

    // Connect button
    let connect_button = Button::with_label("Connect");
    connect_button.add_css_class("secondary");
    connect_button.set_sensitive(peer.status != PeerStatus::Offline);

    let peer_id = peer.id.clone();
    connect_button.connect_clicked(move |button| {
        println!("Connecting to peer: {}", peer_id);
        button.set_label("Disconnect");
    });

    container.append(&info_box);
    container.append(&connect_button);

    container
}
