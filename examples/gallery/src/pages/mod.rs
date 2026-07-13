//! Gallery page modules — one showcase page per Picus component.
//!
//! Layout follows the WinUI Gallery model: the sidebar lists individual
//! controls, and each page focuses on that single control with multiple
//! example cards for variants.

mod basic_input;
mod collections;
mod design;
mod dialogs;
mod layout;
mod menus;
mod status;
mod text;

use bevy_ecs::prelude::*;
use crate::state::GalleryPage;

/// Spawn the content for a gallery page into `parent`.
pub fn spawn_page_content(commands: &mut Commands, parent: Entity, page: GalleryPage) {
    match page {
        GalleryPage::Button => basic_input::spawn_button_page(commands, parent),
        GalleryPage::ToggleSwitch => basic_input::spawn_toggle_switch_page(commands, parent),
        GalleryPage::CheckBox => basic_input::spawn_checkbox_page(commands, parent),
        GalleryPage::RadioButton => basic_input::spawn_radio_button_page(commands, parent),
        GalleryPage::Slider => basic_input::spawn_slider_page(commands, parent),
        GalleryPage::ComboBox => basic_input::spawn_combo_box_page(commands, parent),
        GalleryPage::ColorPicker => basic_input::spawn_color_picker_page(commands, parent),
        GalleryPage::DatePicker => basic_input::spawn_date_picker_page(commands, parent),
        GalleryPage::NumberBox => basic_input::spawn_number_box_page(commands, parent),
        GalleryPage::TextBox => text::spawn_text_box_page(commands, parent),
        GalleryPage::PasswordBox => text::spawn_password_box_page(commands, parent),
        GalleryPage::MultiLineTextBox => text::spawn_multiline_text_box_page(commands, parent),
        GalleryPage::ListView => collections::spawn_list_view_page(commands, parent),
        GalleryPage::TreeView => collections::spawn_tree_view_page(commands, parent),
        GalleryPage::Table => collections::spawn_table_page(commands, parent),
        GalleryPage::DataTable => collections::spawn_data_table_page(commands, parent),
        GalleryPage::MenuBar => menus::spawn_menu_bar_page(commands, parent),
        GalleryPage::TitleBar => menus::spawn_title_bar_page(commands, parent),
        GalleryPage::WindowBackdrop => menus::spawn_window_backdrop_page(commands, parent),
        GalleryPage::ProgressBar => status::spawn_progress_bar_page(commands, parent),
        GalleryPage::Spinner => status::spawn_spinner_page(commands, parent),
        GalleryPage::ToolTip => status::spawn_tooltip_page(commands, parent),
        GalleryPage::Dialog => dialogs::spawn_dialog_page(commands, parent),
        GalleryPage::Toast => dialogs::spawn_toast_page(commands, parent),
        GalleryPage::ContextMenu => dialogs::spawn_context_menu_page(commands, parent),
        GalleryPage::Popover => dialogs::spawn_popover_page(commands, parent),
        GalleryPage::StackPanel => layout::spawn_stack_panel_page(commands, parent),
        GalleryPage::Grid => layout::spawn_grid_page(commands, parent),
        GalleryPage::Responsive => layout::spawn_responsive_page(commands, parent),
        GalleryPage::GroupBox => layout::spawn_group_box_page(commands, parent),
        GalleryPage::SplitPane => layout::spawn_split_pane_page(commands, parent),
        GalleryPage::TabBar => layout::spawn_tab_bar_page(commands, parent),
        GalleryPage::Canvas => layout::spawn_canvas_page(commands, parent),
        GalleryPage::Image => design::spawn_image_page(commands, parent),
        GalleryPage::Icons => design::spawn_icons_page(commands, parent),
        GalleryPage::Shapes => design::spawn_shapes_page(commands, parent),
        GalleryPage::Brushes => design::spawn_brushes_page(commands, parent),
        GalleryPage::Typography => design::spawn_typography_page(commands, parent),
        GalleryPage::Markdown => design::spawn_markdown_page(commands, parent),
        GalleryPage::Theme => design::spawn_theme_page(commands, parent),
        GalleryPage::I18n => design::spawn_i18n_page(commands, parent),
    }
}

pub use dialogs::ManualOverlayMarker;
