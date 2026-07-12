//! Cross-crate cache partition wire format must stay aligned.

#[cfg(test)]
mod tests {
    #[test]
    fn host_core_and_datasets_partition_wire_format_match() {
        let via_core = mei_host_core::partition_cache_key(
            "mini-data",
            "WS-1",
            "cfg-a",
            "inner-key",
        );
        let via_datasets = mei_lang_datasets::partition_cache_key(
            "mini-data",
            "WS-1",
            "cfg-a",
            "inner-key",
        );
        assert_eq!(via_core, via_datasets);
        assert_eq!(
            via_core,
            "part:mini-data/WS-1/cfg-a|inner-key"
        );
    }
}
