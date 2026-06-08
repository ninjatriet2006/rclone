pub mod handle_checksum_select_keys;
pub mod handle_confirm_fallback_keys;
pub mod handle_copy_mode_keys;
pub mod handle_cryptdecode_keys;
pub mod handle_decompress_mode_keys;
pub mod handle_decompress_path_keys;
pub mod handle_dedupe_mode_keys;
pub mod handle_file_view_keys;
pub mod handle_input_key;
pub mod handle_input_paste_rename_keys;
pub mod handle_input_shared_link_keys;
pub mod handle_merge_similar_keys;
pub mod handle_new_folder_popup_keys;
pub mod handle_rename_popup_keys;
pub mod handle_select_base_remote_keys;
pub mod handle_select_remote_keys;
pub mod mod_keys_helper_fallback {
    // placeholder if needed
}
pub mod handle_special_actions_keys;
pub mod handle_tui_selector_keys;
pub mod handle_menu_keys;
pub mod handle_connection_keys;
pub mod handle_explorer_keys;
pub mod handle_services_keys;
pub mod handle_monitor_keys;
pub mod handle_profile_keys;
pub mod handle_language_keys;

pub mod handle_dependency_keys;

pub use handle_checksum_select_keys::handle_checksum_select_keys;
pub use handle_confirm_fallback_keys::handle_confirm_fallback_keys;
pub use handle_copy_mode_keys::handle_copy_mode_keys;
pub use handle_cryptdecode_keys::handle_cryptdecode_keys;
pub use handle_decompress_mode_keys::handle_decompress_mode_keys;
pub use handle_decompress_path_keys::{handle_decompress_path_keys, handle_decompress_path_manual_input_keys};
pub use handle_dedupe_mode_keys::handle_dedupe_mode_keys;
pub use handle_file_view_keys::handle_file_view_keys;
pub use handle_input_key::handle_input_key;
pub use handle_input_paste_rename_keys::handle_input_paste_rename_keys;
pub use handle_input_shared_link_keys::handle_input_shared_link_keys;
pub use handle_merge_similar_keys::{
    handle_merge_similar_destination_select_keys,
    handle_merge_similar_scanning_keys,
    handle_merge_similar_preview_keys,
};
pub use handle_new_folder_popup_keys::handle_new_folder_popup_keys;
pub use handle_rename_popup_keys::handle_rename_popup_keys;
pub use handle_select_base_remote_keys::handle_select_base_remote_keys;
pub use handle_select_remote_keys::handle_select_remote_keys;
pub use handle_special_actions_keys::handle_special_actions_keys;
pub use handle_tui_selector_keys::handle_tui_selector_keys;
pub use handle_menu_keys::handle_menu_keys;
pub use handle_connection_keys::handle_connection_keys;
pub use handle_explorer_keys::handle_explorer_keys;
pub use handle_services_keys::handle_services_keys;
pub use handle_monitor_keys::handle_monitor_keys;
pub use handle_profile_keys::handle_profile_keys;
pub use handle_language_keys::handle_language_keys;
pub use handle_dependency_keys::handle_dependency_keys;
