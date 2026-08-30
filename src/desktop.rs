use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Datelike, Timelike};
use mirrors_edge_save_manager::application_error::UserAction;
use mirrors_edge_save_manager::apply::{ApplyRequest, apply};
use mirrors_edge_save_manager::current_capture::{
    CaptureCurrentRequest, capture_current_as_preset, capture_current_as_stash,
    current_has_verified_match,
};
use mirrors_edge_save_manager::file_dialog::select_save_file;
use mirrors_edge_save_manager::first_activation::{
    ActivateCurrentRequest, activate_current, suggested_current_filename,
};
use mirrors_edge_save_manager::game_process::is_game_running;
use mirrors_edge_save_manager::import_save::{ImportSaveRequest, import_save};
use mirrors_edge_save_manager::locale;
use mirrors_edge_save_manager::overview::{
    ApplicationOverview, CurrentSaveOverview, GameOverview, RecoveryOverview, StoredSaveOverview,
    load_application_overview,
};
use mirrors_edge_save_manager::save_file::SaveFingerprint;
use mirrors_edge_save_manager::storage::StoredSaveRepository;
use mirrors_edge_save_manager::stored_save::{StoredSaveKind, StoredSaveOrigin};
use mirrors_edge_save_manager::stored_save_delete::{delete_all_stashes, delete_stored_save};
use mirrors_edge_save_manager::stored_save_edit::{
    EditStoredSaveRequest, edit_stored_save, promote_stash_to_preset,
};
use slint::winit_030::{EventResult, WinitWindowAccessor, winit};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

slint::include_modules!();

pub fn run() -> Result<(), slint::PlatformError> {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .with_winit_window_attributes_hook(|attributes| {
            attributes.with_theme(Some(winit::window::Theme::Light))
        })
        .select()?;
    let window = AppWindow::new()?;
    window.set_application_version(env!("CARGO_PKG_VERSION").into());
    let repository = StoredSaveRepository::for_current_user().ok();
    let language = repository
        .as_ref()
        .and_then(|repository| repository.preferred_language().ok().flatten())
        .filter(|language| locale::supported(language))
        .unwrap_or_else(|| locale::initial_language().to_owned());
    select_language(&language);
    window.set_language(language.clone().into());
    center_window(&window);
    let weak = window.as_weak();
    window.on_language_requested(move |language| {
        if !locale::supported(language.as_str()) {
            return;
        }
        select_language(language.as_str());
        if let Some(window) = weak.upgrade() {
            window.set_language(language.clone());
        }
        let language = language.to_string();
        std::thread::spawn(move || {
            if let Ok(repository) = StoredSaveRepository::for_current_user() {
                let _ = repository.set_preferred_language(Some(language));
            }
        });
    });
    let weak = window.as_weak();
    window.on_refresh_requested(move || refresh_overview(weak.clone()));
    let game_poll_in_flight = Arc::new(AtomicBool::new(false));
    let weak = window.as_weak();
    window.on_game_poll_requested(move || {
        run_game_poll(weak.clone(), Arc::clone(&game_poll_in_flight))
    });
    let weak = window.as_weak();
    window.on_operation_confirmed(move |id, activation, filename| {
        run_selected_operation(weak.clone(), id, activation, filename)
    });
    let weak = window.as_weak();
    window.on_capture_preflight_requested(move |preset, prefix| {
        run_capture_preflight(weak.clone(), preset, prefix)
    });
    let weak = window.as_weak();
    window.on_capture_requested(move |preset, alias, description| {
        run_current_capture(weak.clone(), preset, alias, description)
    });
    let weak = window.as_weak();
    window.on_import_requested(move || run_import(weak.clone()));
    let weak = window.as_weak();
    window.on_manage_requested(move |operation, id, alias, description| {
        run_manage_operation(weak.clone(), operation, id, alias, description)
    });
    let weak = window.as_weak();
    window.on_clear_stashes_requested(move || run_clear_stashes(weak.clone()));
    let weak = window.as_weak();
    let had_focus = Rc::new(Cell::new(false));
    window.window().on_winit_window_event(move |_, event| {
        if let winit::event::WindowEvent::Focused(focused) = event
            && *focused
            && had_focus.replace(true)
            && let Some(window) = weak.upgrade()
            && window.get_operation_state() != 1
            && window.get_modal_kind() == 0
        {
            refresh_overview(window.as_weak());
        }
        EventResult::Propagate
    });
    refresh_overview(window.as_weak());
    window.run()
}

fn select_language(language: &str) {
    if let Err(error) = slint::select_bundled_translation(language) {
        eprintln!("failed to select bundled language {language}: {error}");
    }
}

#[cfg(windows)]
fn center_window(window: &AppWindow) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

    let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let screen_height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    let window_size = window.window().size();
    let scale_factor = window.window().scale_factor();
    let window_width = if window_size.width > 0 {
        window_size.width as i32
    } else {
        (640.0 * scale_factor) as i32
    };
    let window_height = if window_size.height > 0 {
        window_size.height as i32
    } else {
        (680.0 * scale_factor) as i32
    };
    window.window().set_position(slint::PhysicalPosition::new(
        (screen_width - window_width).max(0) / 2,
        (screen_height - window_height).max(0) / 2,
    ));
}

#[cfg(not(windows))]
fn center_window(_window: &AppWindow) {}

fn refresh_overview(window: slint::Weak<AppWindow>) {
    let language = window
        .upgrade()
        .map(|window| window.get_language().to_string())
        .unwrap_or_else(|| "en".into());
    if let Some(window) = window.upgrade() {
        window.set_selected_save_id(SharedString::default());
        window.set_presets(model(Vec::new()));
        window.set_built_in_presets(model(Vec::new()));
        window.set_stashes(model(Vec::new()));
        window.set_loading(true);
        window.set_game_state(0);
        window.set_game_error_action(-1);
        window.set_recovery_state(0);
        window.set_recovery_error_action(-1);
        window.set_current_state(0);
        window.set_current_error_action(-1);
        window.set_current_source_alias(SharedString::default());
        window.set_current_source_applied(SharedString::default());
        window.set_current_source_kind(0);
        window.set_current_source_changed(false);
        window.set_activation_filename(SharedString::default());
        window.set_activation_error_action(-1);
        window.set_operation_confirming(false);
        window.set_operation_state(0);
        window.set_operation_action(-1);
        window.set_operation_duplicate_count(0);
    }

    std::thread::spawn(move || {
        let overview = load_application_overview();
        let activation = load_activation_suggestion(&overview);
        let _ = window.upgrade_in_event_loop(move |window| {
            apply_overview(window, overview, activation, language)
        });
    });
}

fn run_game_poll(window: slint::Weak<AppWindow>, in_flight: Arc<AtomicBool>) {
    if in_flight.swap(true, Ordering::AcqRel) {
        return;
    }
    let previous_state = window
        .upgrade()
        .map(|window| window.get_game_state())
        .unwrap_or_default();
    std::thread::spawn(move || {
        let result = is_game_running();
        let event_flag = Arc::clone(&in_flight);
        if window
            .upgrade_in_event_loop(move |window| {
                match result {
                    Ok(true) => {
                        window.set_game_state(2);
                        window.set_game_error_action(action_code(UserAction::CloseGame));
                    }
                    Ok(false) if previous_state == 2 => refresh_overview(window.as_weak()),
                    Ok(false) => {
                        window.set_game_state(1);
                        window.set_game_error_action(-1);
                    }
                    Err(_) => {
                        window.set_game_state(3);
                        window.set_game_error_action(action_code(UserAction::Retry));
                    }
                }
                event_flag.store(false, Ordering::Release);
            })
            .is_err()
        {
            in_flight.store(false, Ordering::Release);
        }
    });
}

struct ActivationSuggestion {
    filename: String,
    error_action: i32,
}

fn load_activation_suggestion(overview: &ApplicationOverview) -> ActivationSuggestion {
    if !matches!(overview.current, CurrentSaveOverview::Missing { .. }) {
        return ActivationSuggestion {
            filename: String::new(),
            error_action: -1,
        };
    }

    match suggested_current_filename() {
        Ok(filename) => ActivationSuggestion {
            filename,
            error_action: -1,
        },
        Err(error) => ActivationSuggestion {
            filename: String::new(),
            error_action: action_code(
                mirrors_edge_save_manager::application_error::ApplicationError::from(error)
                    .action(),
            ),
        },
    }
}

fn apply_overview(
    window: AppWindow,
    overview: ApplicationOverview,
    activation: ActivationSuggestion,
    language: String,
) {
    let current_fingerprint = match &overview.current {
        CurrentSaveOverview::Found(current) => Some(current.fingerprint),
        _ => None,
    };
    let last_applied = overview.last_applied.as_ref();
    match overview.game {
        GameOverview::Available => {
            window.set_game_state(1);
            window.set_game_error_action(-1);
        }
        GameOverview::Running => {
            window.set_game_state(2);
            window.set_game_error_action(action_code(UserAction::CloseGame));
        }
        GameOverview::Unavailable(failure) => {
            window.set_game_state(3);
            window.set_game_error_action(action_code(failure.action));
        }
    }

    match overview.recovery {
        RecoveryOverview::Clear => {
            window.set_recovery_state(1);
            window.set_recovery_error_action(-1);
            window.set_recovery_detail(SharedString::default());
        }
        RecoveryOverview::Recovered(count) => {
            window.set_recovery_state(2);
            window.set_recovery_error_action(-1);
            window.set_recovery_detail(count.to_string().into());
        }
        RecoveryOverview::BlockedByGame => {
            window.set_recovery_state(3);
            window.set_recovery_error_action(action_code(UserAction::CloseGame));
            window.set_recovery_detail(SharedString::default());
        }
        RecoveryOverview::Unavailable(failure) => {
            window.set_recovery_state(3);
            window.set_recovery_error_action(action_code(failure.action));
            window.set_recovery_detail(failure.detail.into());
        }
    }

    match overview.current {
        CurrentSaveOverview::Found(current) => {
            window.set_current_state(1);
            window.set_current_error_action(-1);
            window.set_current_modified(
                current
                    .modified_at
                    .map(|time| format_system_time(time, &language))
                    .unwrap_or_default()
                    .into(),
            );
            window.set_current_detail(SharedString::default());
            if let Some(source) = last_applied {
                window.set_current_source_alias(source.alias.clone().into());
                window.set_current_source_applied(
                    format_system_time(source.applied_at, &language).into(),
                );
                window.set_current_source_kind(source_kind(source.kind, source.origin));
                window.set_current_source_changed(current.fingerprint != source.fingerprint);
            } else {
                window.set_current_source_alias(SharedString::default());
                window.set_current_source_applied(SharedString::default());
                window.set_current_source_kind(0);
                window.set_current_source_changed(false);
            }
        }
        CurrentSaveOverview::Missing { directory } => {
            set_missing_current(&window, 2, directory);
        }
        CurrentSaveOverview::SaveDirectoryMissing { directory } => {
            set_missing_current(&window, 3, directory);
        }
        CurrentSaveOverview::Unavailable(failure) => {
            window.set_current_state(4);
            window.set_current_error_action(action_code(failure.action));
            window.set_current_modified(SharedString::default());
            window.set_current_detail(failure.detail.into());
            window.set_current_source_alias(SharedString::default());
            window.set_current_source_applied(SharedString::default());
            window.set_current_source_kind(0);
            window.set_current_source_changed(false);
        }
    }

    let (built_in_presets, presets): (Vec<_>, Vec<_>) = overview
        .stored_saves
        .presets
        .into_iter()
        .partition(|save| save.origin == StoredSaveOrigin::BuiltIn);
    let presets: Vec<_> = presets
        .into_iter()
        .map(|save| save_list_item(save, &language, current_fingerprint))
        .collect();
    let built_in_presets: Vec<_> = built_in_presets
        .into_iter()
        .map(|save| save_list_item(save, &language, current_fingerprint))
        .collect();
    let stashes: Vec<_> = overview
        .stored_saves
        .stashes
        .into_iter()
        .map(|save| save_list_item(save, &language, current_fingerprint))
        .collect();
    if window.get_active_kind() < 0 {
        window.set_active_kind(initial_active_kind(
            !presets.is_empty(),
            !built_in_presets.is_empty(),
        ));
    }
    window.set_presets(model(presets));
    window.set_built_in_presets(model(built_in_presets));
    window.set_stashes(model(stashes));

    if let Some(failure) = overview.stored_saves.failure {
        window.set_library_error_action(action_code(failure.action));
        window.set_library_error_detail(failure.detail.into());
    } else {
        window.set_library_error_action(-1);
        window.set_library_error_detail(SharedString::default());
    }
    window.set_activation_filename(activation.filename.into());
    window.set_activation_error_action(activation.error_action);
    window.set_loading(false);
}

fn run_selected_operation(
    window: slint::Weak<AppWindow>,
    id: SharedString,
    activation: bool,
    confirmed_filename: SharedString,
) {
    if id.is_empty() {
        return;
    }

    if let Some(window) = window.upgrade() {
        begin_operation(&window, i32::from(activation));
    }

    let stored_save_id = id.to_string();
    let confirmed_filename = confirmed_filename.to_string();
    std::thread::spawn(move || {
        let result = execute_selected_operation(&stored_save_id, activation, confirmed_filename);
        finish_operation(window, result);
    });
}

fn run_current_capture(
    window: slint::Weak<AppWindow>,
    preset: bool,
    alias: SharedString,
    description: SharedString,
) {
    if let Some(window) = window.upgrade() {
        begin_operation(&window, if preset { 3 } else { 2 });
        window.set_selected_save_id(SharedString::default());
    }

    std::thread::spawn(move || {
        let result = execute_current_capture(preset, alias.to_string(), description.to_string());
        finish_operation(window, result);
    });
}

fn run_capture_preflight(window: slint::Weak<AppWindow>, preset: bool, prefix: SharedString) {
    if let Some(window) = window.upgrade() {
        begin_operation(&window, if preset { 3 } else { 2 });
    }
    std::thread::spawn(move || {
        let result = StoredSaveRepository::for_current_user()
            .map_err(mirrors_edge_save_manager::application_error::ApplicationError::from)
            .and_then(|repository| {
                current_has_verified_match(&repository, preset)
                    .map_err(mirrors_edge_save_manager::application_error::ApplicationError::from)
            });
        let _ = window.upgrade_in_event_loop(move |window| match result {
            Ok(true) => {
                window.set_operation_duplicate_count(1);
                show_operation_toast(&window, false, -1, if preset { 3 } else { 2 }, 1);
                window.set_operation_state(2);
            }
            Ok(false) => {
                window.set_capture_kind(preset);
                window.set_edit_alias(
                    format!("{} {}", prefix, format_capture_timestamp(SystemTime::now())).into(),
                );
                window.set_edit_description(SharedString::default());
                window.set_operation_state(0);
                window.set_operation_action(-1);
                window.set_modal_kind(4);
            }
            Err(error) => {
                let action = action_code(error.action());
                window.set_operation_action(action);
                show_operation_toast(&window, true, action, if preset { 3 } else { 2 }, 0);
                window.set_operation_state(3);
            }
        });
    });
}

fn run_import(window: slint::Weak<AppWindow>) {
    if let Some(window) = window.upgrade() {
        begin_operation(&window, 4);
        window.set_selected_save_id(SharedString::default());
    }

    std::thread::spawn(move || match select_save_file() {
        Ok(Some(source)) => finish_operation(window, execute_import(source)),
        Ok(None) => {
            let _ = window.upgrade_in_event_loop(|window| window.set_operation_state(0));
        }
        Err(_) => {
            let _ = window.upgrade_in_event_loop(|window| {
                let action = action_code(UserAction::Retry);
                window.set_operation_action(action);
                show_operation_toast(&window, true, action, 4, 0);
                window.set_operation_state(3);
            });
        }
    });
}

fn run_manage_operation(
    window: slint::Weak<AppWindow>,
    operation: i32,
    id: SharedString,
    alias: SharedString,
    description: SharedString,
) {
    if id.is_empty() || !(0..=2).contains(&operation) {
        return;
    }
    if let Some(window) = window.upgrade() {
        begin_operation(&window, operation + 5);
    }
    std::thread::spawn(move || {
        let result =
            execute_manage_operation(operation, &id, alias.to_string(), description.to_string());
        finish_operation(window, result);
    });
}

fn run_clear_stashes(window: slint::Weak<AppWindow>) {
    if let Some(window) = window.upgrade() {
        begin_operation(&window, 8);
        window.set_selected_save_id(SharedString::default());
    }
    std::thread::spawn(move || {
        let result = execute_clear_stashes();
        finish_operation(window, result);
    });
}

fn begin_operation(window: &AppWindow, operation_kind: i32) {
    window.set_toast_visible(false);
    window.set_operation_confirming(false);
    window.set_operation_kind(operation_kind);
    window.set_operation_state(1);
    window.set_operation_action(-1);
    window.set_operation_duplicate_count(0);
}

fn finish_operation(
    window: slint::Weak<AppWindow>,
    result: Result<usize, mirrors_edge_save_manager::application_error::ApplicationError>,
) {
    let language = window
        .upgrade()
        .map(|window| window.get_language().to_string())
        .unwrap_or_else(|| "en".into());
    let overview = load_application_overview();
    let activation_suggestion = load_activation_suggestion(&overview);
    let (operation_state, operation_action, duplicate_count) = match result {
        Ok(duplicate_count) => (2, -1, duplicate_count as i32),
        Err(error) => (3, action_code(error.action()), 0),
    };
    let _ = window.upgrade_in_event_loop(move |window| {
        apply_overview(
            window.clone_strong(),
            overview,
            activation_suggestion,
            language,
        );
        if operation_state == 2 {
            window.set_selected_save_id(SharedString::default());
            window.set_modal_kind(0);
        }
        show_operation_toast(
            &window,
            operation_state == 3,
            operation_action,
            window.get_operation_kind(),
            duplicate_count,
        );
        window.set_operation_state(operation_state);
        window.set_operation_action(operation_action);
        window.set_operation_duplicate_count(duplicate_count);
    });
}

fn show_operation_toast(
    window: &AppWindow,
    error: bool,
    action: i32,
    operation_kind: i32,
    duplicate_count: i32,
) {
    window.set_toast_error(error);
    window.set_toast_action(action);
    window.set_toast_operation_kind(operation_kind);
    window.set_toast_duplicate_count(duplicate_count);
    window.set_toast_visible(true);
}

fn execute_selected_operation(
    stored_save_id: &str,
    activation: bool,
    confirmed_filename: String,
) -> Result<usize, mirrors_edge_save_manager::application_error::ApplicationError> {
    let repository = StoredSaveRepository::for_current_user()?;
    let source = repository
        .list()?
        .into_iter()
        .find(|save| save.id == stored_save_id);
    if activation {
        let result = activate_current(
            &repository,
            ActivateCurrentRequest {
                stored_save_id,
                confirmed_filename,
            },
        )?;
        if let Some(source) = source
            && let Err(error) = repository.record_last_applied(&source, result.fingerprint)
        {
            eprintln!("failed to record Apply source: {error}");
        }
        Ok(0)
    } else {
        let result = apply(
            &repository,
            ApplyRequest {
                stored_save_id,
                automatic_stash_alias: None,
                automatic_stash_description: None,
            },
        )?;
        if let Some(source) = source
            && let Err(error) = repository.record_last_applied(&source, result.applied_fingerprint)
        {
            eprintln!("failed to record Apply source: {error}");
        }
        Ok(result.automatic_stash.duplicate_ids.len())
    }
}

fn execute_current_capture(
    preset: bool,
    alias: String,
    description: String,
) -> Result<usize, mirrors_edge_save_manager::application_error::ApplicationError> {
    let repository = StoredSaveRepository::for_current_user()?;
    let request = CaptureCurrentRequest {
        alias: Some(alias),
        description: (!description.trim().is_empty()).then_some(description),
    };
    let result = if preset {
        capture_current_as_preset(&repository, request)?
    } else {
        capture_current_as_stash(&repository, request)?
    };
    Ok(result.duplicate_ids.len())
}

fn execute_clear_stashes()
-> Result<usize, mirrors_edge_save_manager::application_error::ApplicationError> {
    let repository = StoredSaveRepository::for_current_user()?;
    delete_all_stashes(&repository)?;
    Ok(0)
}

fn execute_import(
    source: std::path::PathBuf,
) -> Result<usize, mirrors_edge_save_manager::application_error::ApplicationError> {
    let repository = StoredSaveRepository::for_current_user()?;
    let result = import_save(
        &repository,
        ImportSaveRequest {
            source,
            alias: None,
            description: None,
        },
    )?;
    Ok(result.duplicate_ids.len())
}

fn execute_manage_operation(
    operation: i32,
    stored_save_id: &str,
    alias: String,
    description: String,
) -> Result<usize, mirrors_edge_save_manager::application_error::ApplicationError> {
    let repository = StoredSaveRepository::for_current_user()?;
    match operation {
        0 => {
            let result = promote_stash_to_preset(&repository, stored_save_id)?;
            return Ok(result.duplicate_ids.len());
        }
        1 => {
            edit_stored_save(
                &repository,
                EditStoredSaveRequest {
                    stored_save_id,
                    alias,
                    description: (!description.trim().is_empty()).then_some(description),
                },
            )?;
        }
        2 => delete_stored_save(&repository, stored_save_id)?,
        _ => return Ok(0),
    }
    Ok(0)
}

fn set_missing_current(window: &AppWindow, state: i32, _directory: std::path::PathBuf) {
    window.set_current_state(state);
    window.set_current_error_action(-1);
    window.set_current_modified(SharedString::default());
    window.set_current_detail(SharedString::default());
    window.set_current_source_alias(SharedString::default());
    window.set_current_source_applied(SharedString::default());
    window.set_current_source_kind(0);
    window.set_current_source_changed(false);
}

fn save_list_item(
    save: StoredSaveOverview,
    language: &str,
    current_fingerprint: Option<SaveFingerprint>,
) -> SaveListItem {
    let matches_current = current_fingerprint.is_some_and(|current| current == save.fingerprint);
    let capture_source_kind = save
        .capture_source
        .as_ref()
        .map(|source| source_kind(source.kind, source.origin))
        .unwrap_or_default();
    let capture_source_alias = save
        .capture_source
        .as_ref()
        .map(|source| source.alias.clone())
        .unwrap_or_default();
    let capture_source_changed = save
        .capture_source
        .as_ref()
        .is_some_and(|source| source.fingerprint != save.fingerprint);
    SaveListItem {
        id: save.id.into(),
        kind: match save.kind {
            StoredSaveKind::Preset => 0,
            StoredSaveKind::Stash => 1,
        },
        alias: save.alias.into(),
        description: save.description.unwrap_or_default().into(),
        created_at: format_system_time(save.created_at, language).into(),
        origin: match save.origin {
            StoredSaveOrigin::BuiltIn => 0,
            StoredSaveOrigin::Current => 1,
            StoredSaveOrigin::Imported => 2,
        },
        matches_current,
        capture_source_kind,
        capture_source_alias: capture_source_alias.into(),
        capture_source_changed,
    }
}

fn source_kind(kind: StoredSaveKind, origin: StoredSaveOrigin) -> i32 {
    match (kind, origin) {
        (StoredSaveKind::Preset, StoredSaveOrigin::BuiltIn) => 2,
        (StoredSaveKind::Preset, _) => 1,
        (StoredSaveKind::Stash, _) => 3,
    }
}

fn initial_active_kind(has_user_presets: bool, has_built_in_presets: bool) -> i32 {
    if !has_user_presets && has_built_in_presets {
        1
    } else {
        0
    }
}

fn model(items: Vec<SaveListItem>) -> ModelRc<SaveListItem> {
    Rc::new(VecModel::from(items)).into()
}

fn action_code(action: UserAction) -> i32 {
    action as i32
}

fn format_system_time(time: SystemTime, language: &str) -> String {
    let Ok(duration) = time.duration_since(UNIX_EPOCH) else {
        return String::new();
    };
    let local = chrono::DateTime::<chrono::Local>::from(
        chrono::DateTime::<chrono::Utc>::from_timestamp(
            duration.as_secs() as i64,
            duration.subsec_nanos(),
        )
        .unwrap_or_default(),
    );
    let year = local.year();
    let month = local.month();
    let day = local.day();
    let hour = local.hour();
    let minute = local.minute();
    let second = local.second();
    let offset = local.offset().to_string();
    if language == "zh-CN" {
        format!("{year:04}年{month:02}月{day:02}日 {hour:02}:{minute:02}:{second:02} {offset}")
    } else {
        format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} {offset}")
    }
}

fn format_capture_timestamp(time: SystemTime) -> String {
    let local = chrono::DateTime::<chrono::Local>::from(time);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        local.year(),
        local.month(),
        local.day(),
        local.hour(),
        local.minute(),
        local.second()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc_timestamp_for_ui() {
        let english = format_system_time(UNIX_EPOCH, "en");
        let chinese = format_system_time(UNIX_EPOCH, "zh-CN");
        let local =
            chrono::DateTime::<chrono::Local>::from(chrono::DateTime::<chrono::Utc>::UNIX_EPOCH);
        assert!(english.contains(&format!(
            "{:04}-{:02}-{:02}",
            local.year(),
            local.month(),
            local.day()
        )));
        assert!(chinese.contains(&format!(
            "{:04}年{:02}月{:02}日",
            local.year(),
            local.month(),
            local.day()
        )));
        assert!(english.contains(":00:00 "));
        assert!(chinese.contains(":00:00 "));
    }

    #[test]
    fn formats_capture_timestamp_without_timezone_suffix() {
        let timestamp = format_capture_timestamp(UNIX_EPOCH);
        assert_eq!(timestamp.len(), 19);
        assert_eq!(&timestamp[4..5], "-");
        assert_eq!(&timestamp[7..8], "-");
        assert_eq!(&timestamp[10..11], " ");
        assert_eq!(&timestamp[13..14], ":");
        assert_eq!(&timestamp[16..17], ":");
    }

    #[test]
    fn initially_opens_built_ins_only_when_user_presets_are_empty() {
        assert_eq!(initial_active_kind(false, true), 1);
        assert_eq!(initial_active_kind(true, true), 0);
        assert_eq!(initial_active_kind(false, false), 0);
    }

    #[test]
    fn apply_is_unavailable_when_save_directory_is_missing() {
        let window = AppWindow::new().expect("create test window");
        window.set_loading(false);
        window.set_game_state(1);
        window.set_recovery_state(1);

        window.set_current_state(3);
        assert!(!window.get_apply_ready());

        window.set_current_state(2);
        assert!(window.get_apply_ready());

        window.set_current_state(1);
        assert!(window.get_apply_ready());

        for language in ["zh-CN", "en", "zh-CN", "en"] {
            select_language(language);
            window.set_language(language.into());
            assert_eq!(window.get_language(), language);
        }
    }
}
