/// 与 `app/assets/tree-icons/icons.svg` 同源；由 shell 注入到页面，
/// `<use href="#i-…"/>` 采用同文档引用。
pub(crate) const TREE_ICONS_SPRITE_SVG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/tree-icons/icons.svg"
));
