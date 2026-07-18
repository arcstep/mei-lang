mod menu_groups;
mod menus;
mod view;

pub use menus::load_topbar_menu_context;
pub use view::AdminNavItem;
pub(crate) use view::{access_scene_for_topbar, topbar_view, ShellNavActive};
