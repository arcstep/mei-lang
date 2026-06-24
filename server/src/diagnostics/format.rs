pub fn format_bytes_human(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let value = bytes as f64;
    if value >= GIB {
        format!("{:.2} GiB", value / GIB)
    } else if value >= MIB {
        format!("{:.1} MiB", value / MIB)
    } else if value >= KIB {
        format!("{:.1} KiB", value / KIB)
    } else {
        format!("{bytes} B")
    }
}

pub fn format_age_ms(recorded_at_ms: u64, now_ms: u64) -> String {
    let elapsed_ms = now_ms.saturating_sub(recorded_at_ms);
    let minutes = elapsed_ms / 60_000;
    if minutes >= 24 * 60 {
        format!("{}d ago", minutes / (24 * 60))
    } else if minutes >= 60 {
        format!("{}h ago", minutes / 60)
    } else if minutes >= 1 {
        format!("{minutes}m ago")
    } else {
        "just now".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_human_units() {
        assert_eq!(format_bytes_human(512), "512 B");
        assert_eq!(format_bytes_human(2048), "2.0 KiB");
        assert_eq!(format_bytes_human(73_506_647), "70.1 MiB");
        assert_eq!(format_bytes_human(4_684_478_208), "4.36 GiB");
    }
}
