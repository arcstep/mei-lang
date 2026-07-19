mod scene_routing;
mod view;

#[cfg(test)]
mod tests;

pub(crate) use scene_routing::access_scene_for_topbar;
pub use view::AdminNavItem;
pub(crate) use view::{topbar_view, ShellNavActive};
