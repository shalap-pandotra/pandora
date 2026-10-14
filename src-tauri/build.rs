fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
        "set_library_path",
        "get_authors",
        "get_series",
        "get_tags",
        "get_books_filtered",
        "get_multi_text_columns",
        "get_comments",
        "get_next_series_index",
        "save_metadata_batch",
        "get_custom_editor",
        "get_edit_extras",
        "preview_cover",
        "save_last_library_path",
        "load_last_library_path",
        "open_book",
        "reader_load",
        "reader_settings",
        "set_reader_settings",
        "reader_position",
        "save_reader_position",
        "reader_library_books",
        "reader_replace",
        "reader_revert",
    ])))
    .expect("failed to run tauri-build")
}
