//! Graph registry, CAS, import-bundle, and v2 assemble for mei-host-shell.

mod artifact_biz_macros;
mod assemble;
mod bridge;
mod content_store;
mod data_snapshot;
mod enrich_compiled_scope;
mod import;
mod io;
mod eval_slot_group;
mod layer_overlay;
mod layer_plan;
mod layer_store;
mod structure_full;
mod view_artifact;
mod layout_tuning_merge;
mod mcg;
mod metric_hydrate;
mod mrg;
mod panel_constants;
mod paths;
mod presentation_map;
mod projection_normalize;
mod semantic_cache;
mod tier;
mod types;
mod v2_bundle_constants;
mod v2_lower;
mod v2_metric_lower;
mod world_plan;

pub use assemble::{
    assemble_scope_from_registry, collect_all_board_scenes, list_scope_routes, AssembleOutcome,
    ScopeRoute,
};
pub use enrich_compiled_scope::{
    enrich_compiled_scope, EnrichCompiledScopeOptions,
};
pub use data_snapshot::{
    collect_app_xlsx_sources, publish_app_data_snapshots, PublishDataSnapshotsReport,
};
pub use import::{import_bundle, load_block_artifact, ImportOptions};
pub use eval_slot_group::{
    build_eval_slot_group_document, collect_slot_groups, ensure_eval_slot_group_cached,
    persist_eval_slot_group, EvalSlotGroupDocument, EVAL_SLOT_GROUP_SCHEMA,
};
pub use layer_overlay::{
    default_theme_tokens, ensure_layout_overlay_cached, ensure_theme_tokens_cached,
    layout_overlay_from_draft, persist_layout_overlay, persist_theme_tokens,
    LayoutOverlayDocument, ThemeTokensDocument, LAYOUT_OVERLAY_SCHEMA, THEME_TOKENS_SCHEMA,
};
pub use layer_store::{clear_layers_for_app, layer_entry_meta, store_layer, take_layer};
pub use structure_full::{
    build_structure_full_document, build_structure_index_document, nodes_within_projection,
    persist_structure_full, slot_group_id_for_node, structure_full_from_compiled,
    ui_role_depth_rank,
};
pub use view_artifact::{
    build_semantic_core_for_scene, eval_slot_group_cache_key, layout_overlay_persisted_cache_key,
    layout_overlay_session_cache_key, manifest_revision_digest, shell_cache_key,
    structure_full_cache_key, theme_tokens_cache_key, ComposeRequest, LayerRef,
    SceneViewManifest, StructureFullDocument, StructureFullNode, WysiwygPanelPatch,
    EVAL_SLOT_GROUP_KIND, LAYOUT_OVERLAY_KIND, SCENE_VIEW_MANIFEST_SCHEMA,
    STRUCTURE_FULL_KIND, STRUCTURE_FULL_SCHEMA, THEME_TOKENS_KIND,
};
pub use layout_tuning_merge::merge_layout_tuning_into_compiled;
pub use mcg::registry::{McgRegistry, McgRegistryWriter};
pub use mrg::eval_cache_plan::{
    build_eval_cache_invalidation_plan, build_eval_cache_invalidation_plan_from_registry,
    invalidate_app_eval_cache,
};
pub use mrg::client_bootstrap::{
    bootstrap_embed_allowed, bootstrap_embed_status, bootstrap_embed_status_for_manifest,
    build_client_bootstrap_head_fragment, build_client_bootstrap_payload, BootstrapEmbedStatus,
    clear_client_bootstrap_for_scope, clear_client_bootstraps_for_stale_scopes,
    client_bootstrap_path, empty_client_bootstrap_payload, read_client_bootstrap,
    read_scene_bootstrap_artifact, scene_bootstrap_artifact_public_url,
    scene_requires_client_bootstrap, write_client_bootstrap, write_scene_bootstrap_artifact,
    ClientBootstrapManifest, ClientBootstrapPayload, ClientBootstrapScopePayload,
    NO_CLIENT_BOOTSTRAP_REVISION,
};
pub use mrg::frontier::{
    collect_eval_frontier, collect_eval_frontier_with_hops, linked_board_scenes_for_scope,
    record_navigation_edges_for_scope, FrontierMetric,
};
pub use mrg::registry::{MrgRegistry, MrgRegistryWriter};
pub use mrg::slots::{
    default_metric_response_descriptor, mark_slots_stale_for_bundles, record_slot_failed,
    record_slot_from_descriptor, record_slots_from_descriptors, MRG_REGISTRY_SCHEMA_V3,
};
pub use mrg::telemetry::{
    flush_telemetry_to_registry, mrg_status_json, record_access, record_scope_activation,
    MrgAccessKind,
};
pub use mrg::tier::{compute_client_revision, WarmupTier};
pub use mrg::warmup::{record_navigation_edge, warm_frontier_slots, WarmupFrontierOutcome};
pub use paths::{bridge_path, mcg_registry_path, mrg_registry_path, resolve_graph_root};
pub use presentation_map::{
    build_presentation_map, presentation_map_to_value, resolve_viewpoint_id,
    PresentationMapDocument,
};
pub use semantic_cache::{
    build_page_render_view_axes, build_semantic_cache_core, page_render_view_signature,
    semantic_cache_core_signature, PageRenderViewAxes, SemanticCacheCore,
};
pub use tier::{
    canonical_tier, default_z_index_for_chrome_role, default_z_index_for_tier,
    resolve_stack_order, runtime_overlay_z_index, z_index_in_named_plane, z_index_in_tier_band,
    compute_panel_z_index, DEFAULT_PANEL_TIER, STACK_ORDER_MAX, Z_T1_HEADER, TIER_T0, TIER_T1,
    TIER_T2,
};
pub use content_store::{
    content_hash_bytes, get, put_if_absent, read_payload_json, resolve_payload_ref,
    APP_SKELETON, METRIC_DEF_BUNDLE, METRIC_RESPONSE, NAVIGATION, PANEL_CONTRACT,
    PROJECTION_ASSEMBLY, WARMUP_POLICY,
};
pub use types::{GraphNodeId, GraphNodeKind, MaterialState, PayloadRef};
pub use world_plan::{build_world_exchange, build_map_projection, build_world_plan, WorldCompileOutcome};
