//! Graph registry, CAS, import-bundle, and v2 assemble for mei-host-shell.

mod artifact_biz_macros;
mod assemble;
mod assemble_cache;
mod bridge;
mod compose_chrome;
mod content_store;
mod data_snapshot;
mod enrich_compiled_scope;
mod eval_slot_group;
mod hierarchy_spacing;
mod import;
mod io;
mod layer_overlay;
mod layer_plan;
mod layer_store;
mod manifest_index;
mod mcg;
mod metric_hydrate;
mod mrg;
mod panel_constants;
mod panel_scope_resolve;
mod paths;
mod presentation_map;
mod projection_normalize;
mod runtime_plans;
mod scene_materialize;
mod scope_target;
mod semantic_cache;
mod semantic_scene;
mod shell_layer;
mod stage_bootstrap;
mod structure_full;
mod surface;
mod theme_layout_merge;
mod tier;
mod types;
mod v2_bundle_constants;
mod v2_lower;
mod v2_metric_lower;
mod view_artifact;
mod view_layer_warmup;
mod warmup_last_run;
mod world_plan;

pub use assemble::{
    assemble_scope_from_registry, collect_all_t2_page_scenes, list_scope_routes,
    t2_page_scenes_for_section_scope, AssembleOutcome, ScopeRoute,
};
pub use assemble_cache::{
    assemble_cache_key, assemble_cache_key_partitioned, clear_assemble_cache_for_app,
    clear_assemble_cache_for_partition, store_assemble_outcome, take_assemble_outcome,
};
pub use content_store::{
    content_hash_bytes, get, put_if_absent, read_payload_bytes, read_payload_json,
    resolve_payload_ref, APP_SKELETON, CONTENT_PANEL, METRIC_DEF_BUNDLE, METRIC_RESPONSE,
    NAVIGATION, PROJECTION_ASSEMBLY, WARMUP_POLICY,
};
pub use data_snapshot::{
    collect_app_xlsx_sources, publish_app_data_snapshots, PublishDataSnapshotsReport,
};
pub use enrich_compiled_scope::{enrich_compiled_scope, EnrichCompiledScopeOptions};
pub use eval_slot_group::{
    build_eval_slot_group_document, collect_slot_groups, ensure_eval_slot_group_cached,
    persist_eval_slot_group, EvalSlotGroupDocument, EVAL_SLOT_GROUP_SCHEMA,
};
pub use import::{import_bundle, load_block_artifact, ImportOptions};
pub use layer_overlay::{
    default_theme_tokens, ensure_layout_overlay_cached, ensure_theme_tokens_cached,
    layout_overlay_from_draft, persist_layout_overlay, persist_theme_tokens, LayoutOverlayDocument,
    ThemeTokensDocument, LAYOUT_OVERLAY_SCHEMA, THEME_TOKENS_SCHEMA,
};
pub use layer_store::{
    clear_layers_for_app, clear_layers_for_partition, layer_entry_meta, store_layer,
    store_layer_partitioned, take_layer, take_layer_partitioned,
};
pub use manifest_index::{
    clear_manifest_index_for_app, clear_manifest_index_for_partition,
    load_manifest_index_from_content_store, manifest_index_cache_key,
    manifest_index_cache_key_partitioned, manifest_index_to_scene_manifest, persist_manifest_index,
    store_manifest_index_memory, take_manifest_index, ManifestIndexDocument, SurfaceManifestSlice,
    MANIFEST_INDEX_KIND, MANIFEST_INDEX_SCHEMA,
};
pub use mcg::registry::{
    McgNodeRecord, McgRegistry, McgRegistryWriter, MCG_REGISTRY_SCHEMA_VERSION,
};
pub use mrg::client_bootstrap::{
    bootstrap_embed_allowed, bootstrap_embed_status, bootstrap_embed_status_for_manifest,
    build_client_bootstrap_head_fragment, build_client_bootstrap_payload,
    clear_client_bootstrap_for_scope, clear_client_bootstraps_for_stale_scopes,
    client_bootstrap_eval_seed_json, client_bootstrap_pack_candidate_scopes, client_bootstrap_path,
    client_bootstrap_scope_allowed, delivery_class_counts_for_scope,
    empty_client_bootstrap_payload, read_client_bootstrap, read_scene_bootstrap_artifact,
    scene_bootstrap_artifact_public_url, scene_requires_client_bootstrap, write_client_bootstrap,
    write_scene_bootstrap_artifact, BootstrapEmbedStatus, ClientBootstrapManifest,
    ClientBootstrapPayload, ClientBootstrapScopePayload, NO_CLIENT_BOOTSTRAP_REVISION,
};
pub use mrg::eval_cache_plan::{
    build_eval_cache_invalidation_plan, build_eval_cache_invalidation_plan_from_registry,
    invalidate_app_eval_cache,
};
pub use mrg::frontier::{
    collect_eval_frontier, collect_eval_frontier_with_hops, linked_t2_page_pack_scopes,
    linked_t2_page_scenes_for_scope, record_navigation_edges_for_scope, FrontierMetric,
};
pub use mrg::registry::{MrgRegistry, MrgRegistryWriter};
pub use mrg::scene_eval_pack::{
    build_scene_eval_pack, SceneEvalPackBuildOptions, SceneEvalPackEvalLayerRef,
    SceneEvalPackResponse, SceneEvalPackStatus,
};
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
    build_presentation_map, presentation_map_to_value, resolve_viewpoint_id, PresentationDeck,
    PresentationDeckSlide, PresentationMapDocument,
};
pub use runtime_plans::{
    build_runtime_plans_document, empty_runtime_plans_document, ensure_runtime_plans_cached,
    persist_runtime_plans, runtime_plans_cache_key, runtime_plans_from_outcome,
    RuntimePlansDocument, RUNTIME_PLANS_KIND, RUNTIME_PLANS_SCHEMA,
};
pub use scene_materialize::{
    build_scene_view_manifest, ensure_manifest_index, ensure_scoped_manifest_index,
    manifest_for_surface, materialize_layers_for_request, resolve_view_revision_for_surface,
    ArtifactHitMatrix, ShellChromeRenderArgs, ShellChromeRenderer,
};
pub use scope_target::{
    canonical_scoped_path, canonical_t2_page_path, canonical_temp_stage_path,
    infer_stage_from_temp_target, parse_scoped_route_tail, parse_temp_stage_target,
    resolve_scope_target, ScopeTarget, ScopeTargetHint, ScopeTargetResolveError, ScopedRouteParse,
    SCOPED_ROUTE_ROLES,
};
pub use semantic_cache::{
    build_page_render_view_axes, build_semantic_cache_core, page_render_view_signature,
    semantic_cache_core_signature, PageRenderViewAxes, SemanticCacheCore,
};
pub use semantic_scene::{
    assemble_semantic_scene, collect_world_payloads_from_scene, default_target_for_scene,
    has_semantic_scene, load_semantic_scene_payload, target_key_from_payload,
    SemanticSceneAssembly,
};
pub use stage_bootstrap::{
    narration_catalogs_bootstrap, stage_programs_bootstrap, stage_registry_bootstrap,
};
pub use shell_layer::{
    build_shell_layer_document, ensure_shell_layer_cached, ensure_shell_layer_rendered,
    is_placeholder_shell_document, shell_layer_json, store_shell_layer_document,
    ShellLayerDocument, SHELL_LAYER_SCHEMA,
};
pub use structure_full::{
    build_structure_full_document, build_structure_index_document, closure_for_node_id,
    closure_for_preview_scope, nodes_within_projection, persist_structure_full,
    slot_group_id_for_node, structure_full_from_compiled, ui_role_depth_rank,
};
pub use surface::{
    apply_surface_to_props, normalize_surface, surface_chrome_props, surface_field_layout_call,
    surface_template_name,
};
pub use tier::{
    canonical_tier, compute_panel_z_index, default_z_index_for_chrome_role,
    default_z_index_for_tier, resolve_stack_order, runtime_overlay_z_index, z_index_in_named_plane,
    z_index_in_tier_band, DEFAULT_PANEL_TIER, STACK_ORDER_MAX, TIER_T0, TIER_T1, TIER_T2,
    Z_T1_HEADER,
};
pub use types::{GraphNodeId, GraphNodeKind, MaterialState, PayloadRef};
pub use view_artifact::{
    build_semantic_core_for_scene, build_semantic_core_for_scene_scoped,
    collect_manifest_layer_refs, eval_slot_group_cache_key, layer_ref_from_manifest_entry,
    layout_overlay_persisted_cache_key, layout_overlay_session_cache_key, manifest_revision_digest,
    resolve_view_revision, semantic_revision_digest, shell_cache_key, structure_full_cache_key,
    surface_revision_digest_from_manifest, theme_tokens_cache_key, AssemblyPlan,
    ClientLayerHolding, ComposeRequest, FrameViewportMeta, LayerRef, SceneViewManifest,
    StructureFullDocument, StructureFullNode, ViewRevisionInput, ViewRevisionResponse,
    ViewRevisionStatus, WysiwygPanelPatch, EVAL_SLOT_GROUP_KIND, LAYOUT_OVERLAY_KIND,
    SCENE_VIEW_MANIFEST_SCHEMA, STRUCTURE_FULL_KIND, STRUCTURE_FULL_SCHEMA, THEME_TOKENS_KIND,
};
pub use view_layer_warmup::{warm_manifest_index_for_app, warm_manifest_index_for_scope};
pub use warmup_last_run::{
    current_time_ms as warmup_last_run_time_ms, read_warmup_last_run, warmup_last_run_json,
    write_warmup_last_run, WarmupLastRunRecord, WARMUP_LAST_RUN_REL,
};
pub use world_plan::{
    build_map_projection, build_world_exchange, build_world_plan, WorldCompileOutcome,
};
