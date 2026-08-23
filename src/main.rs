#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use mirrors_edge_save_manager::application_error::UserAction;
use mirrors_edge_save_manager::apply::{ApplyRequest, apply};
use mirrors_edge_save_manager::current_capture::{
    CaptureCurrentRequest, capture_current_as_preset, capture_current_as_stash,
};
use mirrors_edge_save_manager::file_dialog::select_save_file;
use mirrors_edge_save_manager::first_activation::{
    ActivateCurrentRequest, activate_current, suggested_current_filename,
};
use mirrors_edge_save_manager::game_process::is_game_running;
use mirrors_edge_save_manager::import_save::{ImportSaveRequest, import_save};
use mirrors_edge_save_manager::overview::{
    ApplicationOverview, CurrentSaveOverview, GameOverview, RecoveryOverview, StoredSaveOverview,
    load_application_overview,
};
use mirrors_edge_save_manager::storage::StoredSaveRepository;
use mirrors_edge_save_manager::stored_save::{StoredSaveKind, StoredSaveOrigin};
use mirrors_edge_save_manager::stored_save_delete::delete_stored_save;
use mirrors_edge_save_manager::stored_save_edit::{
    EditStoredSaveRequest, edit_stored_save, promote_stash_to_preset,
};
use slint::winit_030::{EventResult, WinitWindowAccessor, winit};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let window = AppWindow::new()?;
    center_window(&window);
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
    window.on_capture_requested(move |preset| run_current_capture(weak.clone(), preset));
    let weak = window.as_weak();
    window.on_import_requested(move || run_import(weak.clone()));
    let weak = window.as_weak();
    window.on_manage_requested(move |operation, id, alias, description| {
        run_manage_operation(weak.clone(), operation, id, alias, description)
    });
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
    if let Some(window) = window.upgrade() {
        window.set_selected_save_id(SharedString::default());
        window.set_presets(model(Vec::new()));
        window.set_stashes(model(Vec::new()));
        window.set_loading(true);
        window.set_game_state(0);
        window.set_game_error_action(-1);
        window.set_recovery_state(0);
        window.set_recovery_error_action(-1);
        window.set_current_state(0);
        window.set_current_error_action(-1);
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
        let _ = window
            .upgrade_in_event_loop(move |window| apply_overview(window, overview, activation));
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
) {
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
                    .map(format_system_time)
                    .unwrap_or_default()
                    .into(),
            );
            window.set_current_detail(SharedString::default());
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
        }
    }

    let presets = overview
        .stored_saves
        .presets
        .into_iter()
        .map(save_list_item)
        .collect();
    let stashes = overview
        .stored_saves
        .stashes
        .into_iter()
        .map(save_list_item)
        .collect();
    window.set_presets(model(presets));
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

fn run_current_capture(window: slint::Weak<AppWindow>, preset: bool) {
    if let Some(window) = window.upgrade() {
        begin_operation(&window, if preset { 3 } else { 2 });
        window.set_selected_save_id(SharedString::default());
    }

    std::thread::spawn(move || {
        let result = execute_current_capture(preset);
        finish_operation(window, result);
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
                window.set_operation_state(3);
                window.set_operation_action(action_code(UserAction::Retry));
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

fn begin_operation(window: &AppWindow, operation_kind: i32) {
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
    let overview = load_application_overview();
    let activation_suggestion = load_activation_suggestion(&overview);
    let (operation_state, operation_action, duplicate_count) = match result {
        Ok(duplicate_count) => (2, -1, duplicate_count as i32),
        Err(error) => (3, action_code(error.action()), 0),
    };
    let _ = window.upgrade_in_event_loop(move |window| {
        apply_overview(window.clone_strong(), overview, activation_suggestion);
        if operation_state == 2 {
            window.set_selected_save_id(SharedString::default());
            window.set_modal_kind(0);
        }
        window.set_operation_state(operation_state);
        window.set_operation_action(operation_action);
        window.set_operation_duplicate_count(duplicate_count);
    });
}

fn execute_selected_operation(
    stored_save_id: &str,
    activation: bool,
    confirmed_filename: String,
) -> Result<usize, mirrors_edge_save_manager::application_error::ApplicationError> {
    let repository = StoredSaveRepository::for_current_user()?;
    if activation {
        activate_current(
            &repository,
            ActivateCurrentRequest {
                stored_save_id,
                confirmed_filename,
            },
        )?;
    } else {
        apply(
            &repository,
            ApplyRequest {
                stored_save_id,
                automatic_stash_alias: None,
                automatic_stash_description: None,
            },
        )?;
    }
    Ok(0)
}

fn execute_current_capture(
    preset: bool,
) -> Result<usize, mirrors_edge_save_manager::application_error::ApplicationError> {
    let repository = StoredSaveRepository::for_current_user()?;
    let request = CaptureCurrentRequest {
        alias: None,
        description: None,
    };
    let result = if preset {
        capture_current_as_preset(&repository, request)?
    } else {
        capture_current_as_stash(&repository, request)?
    };
    Ok(result.duplicate_ids.len())
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
            promote_stash_to_preset(&repository, stored_save_id)?;
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
}

fn save_list_item(save: StoredSaveOverview) -> SaveListItem {
    SaveListItem {
        id: save.id.into(),
        kind: match save.kind {
            StoredSaveKind::Preset => 0,
            StoredSaveKind::Stash => 1,
        },
        alias: save.alias.into(),
        description: save.description.unwrap_or_default().into(),
        created_at: format_system_time(save.created_at).into(),
        origin: match save.origin {
            StoredSaveOrigin::BuiltIn => 0,
            StoredSaveOrigin::Current => 1,
            StoredSaveOrigin::Imported => 2,
        },
    }
}

fn model(items: Vec<SaveListItem>) -> ModelRc<SaveListItem> {
    Rc::new(VecModel::from(items)).into()
}

fn action_code(action: UserAction) -> i32 {
    action as i32
}

fn format_system_time(time: SystemTime) -> String {
    let Ok(duration) = time.duration_since(UNIX_EPOCH) else {
        return String::new();
    };
    let seconds = duration.as_secs() as i64;
    let days = seconds / 86_400;
    let seconds_in_day = seconds % 86_400;
    let hour = seconds_in_day / 3_600;
    let minute = seconds_in_day % 3_600 / 60;
    let (year, month, day) = civil_date_from_days(days);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02} UTC")
}

fn civil_date_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let days = days_since_epoch + 719_468;
    let era = days / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_position = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_position + 2) / 5 + 1;
    let month = month_position + if month_position < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc_timestamp_for_ui() {
        assert_eq!(format_system_time(UNIX_EPOCH), "1970-01-01 00:00 UTC");
        assert_eq!(
            format_system_time(UNIX_EPOCH + std::time::Duration::from_secs(1_785_715_200)),
            "2026-08-03 00:00 UTC"
        );
    }
}
