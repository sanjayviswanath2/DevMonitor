mod activity;
mod commands;
mod doctor;
mod system;

use system::AppState;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // sysinfo keeps /proc/<pid>/stat open by default; unbounded that can mean
    // thousands of FDs and a frozen GTK main loop on busy developer machines.
    let _ = sysinfo::set_open_files_limit(32);

    // Set DEVMONITOR_NO_TRAY=1 to skip tray (diagnostic / fallback).
    let enable_tray = std::env::var_os("DEVMONITOR_NO_TRAY").is_none();

    tauri::Builder::default()
        .manage(AppState::new())
        .setup(move |app| {
            if enable_tray {
                let show_i =
                    MenuItem::with_id(app, "show", "Open DevMonitor", true, None::<&str>)?;
                let quit_i =
                    MenuItem::with_id(app, "quit", "Quit DevMonitor", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

                let mut tray = TrayIconBuilder::with_id("devmonitor-tray")
                    .menu(&menu)
                    .tooltip("DevMonitor — running in background")
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "show" => show_main_window(app),
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            show_main_window(tray.app_handle());
                        }
                    });

                if let Some(icon) = app.default_window_icon() {
                    tray = tray.icon(icon.clone());
                }

                let _tray = tray.build(app)?;
            }

            activity::start_collector();
            Ok(())
        })
        .on_window_event(move |window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if enable_tray {
                    // Closing the window hides to tray; activity collector keeps running.
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_system_stats,
            commands::enable_swap,
            commands::drop_memory_caches,
            commands::clear_developer_storage,
            commands::get_processes,
            commands::get_listening_ports,
            commands::kill_process,
            commands::pick_project_folder,
            commands::inspect_project,
            commands::scan_issues,
            commands::free_port,
            commands::switch_node,
            commands::list_node_versions,
            commands::use_node_version,
            commands::repair_node_path,
            commands::repair_npm_permissions,
            commands::clean_reinstall_deps,
            commands::apply_node_heap,
            commands::fix_file_watchers,
            commands::list_safe_caches,
            commands::clean_caches,
            commands::get_node_environment,
            commands::get_docker_disk,
            commands::prune_docker,
            commands::check_native_toolchain,
            commands::install_build_tools,
            commands::get_python_environment,
            commands::get_git_status,
            commands::set_git_identity,
            commands::diagnose_git_ssh,
            commands::use_git_host_account,
            commands::use_git_host_token,
            commands::open_git_token_page,
            commands::remove_git_host_account,
            commands::install_or_update_git,
            commands::uninstall_git,
            commands::clear_git_identity,
            commands::store_git_credentials,
            commands::test_git_host_connection,
            commands::scan_env_gaps,
            commands::get_activity_day,
            commands::get_activity_settings,
            commands::set_activity_settings,
            commands::clear_activity_history,
            commands::list_activity_dates,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
