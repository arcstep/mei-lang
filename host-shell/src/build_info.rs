pub const BUILD_VERSION: &str = env!("MEI_BUILD_VERSION");

pub fn fill_host_build_placeholders(mut html: String) -> String {
    html = html.replace("__MEI_HOST_VERSION__", BUILD_VERSION);
    html = html.replace("__MEI_HOST_VERSION_LABEL__", BUILD_VERSION);
    html
}
