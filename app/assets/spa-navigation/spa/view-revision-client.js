/**
 * Client view-revision negotiation: revision-first layer assembly gate.
 */
(function initViewRevisionClient(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  const VIEW_REVISION_API = "/api/host/view-revision";

  const ViewRevisionOutcome = {
    REFETCH: "refetch",
    ASSEMBLE_LOCAL: "assemble_local",
    LOCAL_MISS: "local_miss",
  };

  function isViewRevisionEnabled() {
    return globalThis.__mei?.view_revision_enabled !== false;
  }

  function holdingsFromLayerCache(holdings) {
    return (holdings || [])
      .map((row) => ({
        name: String(row?.name || "").trim(),
        artifact_id: String(row?.artifact_id || "").trim(),
        content_hash: String(row?.content_hash || "").trim(),
      }))
      .filter((row) => row.name && row.artifact_id && row.content_hash);
  }

  function revisionsMatchManifest(manifestDigest, localDigest) {
    const a = String(manifestDigest || "").trim();
    const b = String(localDigest || "").trim();
    if (!a || !b) return false;
    return a === b;
  }

  function readDocumentRevisionEnvelope() {
    const envelope = globalThis.__mei?.view_revision_envelope;
    if (envelope && typeof envelope === "object") return envelope;
    const refs = globalThis.__mei?.scene_manifest_refs;
    if (!refs || typeof refs !== "object") return null;
    return {
      app_id: refs.app_id || "",
      scene_id: refs.scene_id || "",
      manifest_revision_digest: refs.revision_digest || refs.manifest_revision_digest || "",
      surface_revision_digest: refs.surface_revision_digest || "",
    };
  }

  function documentEnvelopeMatchesStored(ctx, stored) {
    const envelope = readDocumentRevisionEnvelope();
    if (!envelope || !stored) return false;
    const appId = String(ctx.app_id || ctx.appId || "").trim();
    const sceneId = String(ctx.scene_id || ctx.sceneId || "home").trim();
    if (envelope.app_id && String(envelope.app_id) !== appId) return false;
    if (envelope.scene_id && String(envelope.scene_id) !== sceneId) return false;
    return (
      revisionsMatchManifest(
        envelope.manifest_revision_digest || envelope.revision_digest,
        stored.manifest_revision_digest,
      ) &&
      revisionsMatchManifest(
        envelope.surface_revision_digest,
        stored.surface_revision_digest,
      )
    );
  }

  function layerRefFromManifestValue(layerName, value) {
    if (!value || typeof value !== "object") return null;
    const artifactId = String(value.artifact_id || "").trim();
    const contentHash = String(value.content_hash || "").trim();
    if (artifactId && contentHash) {
      return { name: layerName, artifact_id: artifactId, content_hash: contentHash };
    }
    return null;
  }

  function layerRefsFromManifest(manifest) {
    const refs = [];
    const layers = manifest?.layers || {};
    for (const [name, value] of Object.entries(layers)) {
      const ref = layerRefFromManifestValue(name, value);
      if (ref) refs.push(ref);
    }
    return refs;
  }

  function defaultReviewProjectionForSurface(_surface) {
    // Legacy layout/prototype map to app defaults; prefer manifest compose_defaults.
    return "live_full";
  }

  function defaultDataModeForSurface(_surface) {
    return "eval";
  }

  function buildComposeRequest(ctx) {
    const payload = ctx || {};
    const refsDefaults = globalThis.__mei?.scene_manifest_refs?.compose_defaults;
    const defaultTab =
      boot.sceneManifestLoader?.defaultTabForSurface || (() => "scene");
    const tab = String(payload.tab || "").trim() || defaultTab("app");
    const reviewFromCtx = String(
      payload.review_projection || payload.reviewProjection || "",
    ).trim();
    const dataFromCtx = String(payload.data_mode || payload.dataMode || "").trim();
    return {
      route_mode: "app",
      tab,
      chrome: String(payload.chrome || refsDefaults?.chrome || "").trim(),
      review_projection:
        reviewFromCtx ||
        String(refsDefaults?.review_projection || "").trim() ||
        defaultReviewProjectionForSurface("app"),
      data_mode:
        dataFromCtx ||
        String(refsDefaults?.data_mode || "").trim() ||
        defaultDataModeForSurface("app"),
      focus: String(payload.focus || refsDefaults?.focus || "").trim(),
      scope: String(payload.scope || refsDefaults?.scope || "").trim(),
    };
  }

  function composeDefaultsFromResponse(response, ctx) {
    return (
      response?.compose_defaults ||
      response?.assembly_plan?.compose_defaults ||
      response?.manifest?.compose_defaults ||
      globalThis.__mei?.scene_manifest_refs?.compose_defaults ||
      buildComposeRequest(ctx)
    );
  }

  async function fetchViewRevision(ctx, options) {
    const opts = options || {};
    if (!isViewRevisionEnabled()) {
      return { ready: false, status: ViewRevisionOutcome.REFETCH, disabled: true };
    }
    const params = new URLSearchParams({
      app_id: ctx.app_id || ctx.appId || "",
      scene: ctx.scene_id || ctx.sceneId || "home",
      surface: "app",
    });
    const compose = buildComposeRequest(ctx);
    params.set("compose", JSON.stringify(compose));
    if (ctx.node) params.set("node", ctx.node);
    if (opts.recover || opts.local_miss) {
      params.set("recover", "1");
    } else if (!opts.omit_digests) {
      const digests =
        opts.client_digests ||
        (boot.readClientDigests ? boot.readClientDigests(ctx) : null) ||
        {};
      const manifestDigest = String(digests.manifest_revision_digest || "").trim();
      const surfaceDigest = String(digests.surface_revision_digest || "").trim();
      if (manifestDigest) params.set("manifest_revision_digest", manifestDigest);
      if (surfaceDigest) params.set("surface_revision_digest", surfaceDigest);
    }
    const headers = {
      ...(boot.clientCommandHeaders ? boot.clientCommandHeaders("REVISION", "view-revision") : {}),
    };
    if (ctx.draft_session) {
      headers["x-mei-draft-session"] = ctx.draft_session;
    }
    const response = await global.fetch(`${VIEW_REVISION_API}?${params.toString()}`, {
      credentials: "same-origin",
      headers,
      signal: opts.signal,
    });
    if (!response.ok) {
      throw new Error(`view-revision ${response.status}`);
    }
    const payload = await response.json();
    payload._headers = {
      status: response.headers.get("x-mei-view-revision-status") || payload.status,
      assemble_local: response.headers.get("x-mei-assemble-local") === "1",
      local_miss: response.headers.get("x-mei-local-miss") === "1",
    };
    return payload;
  }

  function extractLayerDocument(layerValue) {
    if (layerValue == null) return null;
    if (typeof layerValue === "object" && layerValue.document != null) {
      return layerValue.document;
    }
    return layerValue;
  }

  async function storeInlineLayers(ctx, inlineLayers, manifest) {
    if (!inlineLayers || !boot.layerStore) return;
    const entries = [];
    for (const [name, bytes] of Object.entries(inlineLayers)) {
      const ref = layerRefFromManifestValue(name, manifest?.layers?.[name]);
      if (!ref) continue;
      const document = extractLayerDocument(bytes);
      entries.push({ holding: ref, bytes: document });
    }
    await boot.layerStore.putLayersByRef?.(
      ctx.app_id || ctx.appId,
      ctx.scene_id || ctx.sceneId,
      entries,
      manifest,
      { awaitPersist: false },
    );
  }

  function planFromValidatedStored(ctx, response) {
    const stored = boot.readViewRevision?.(ctx);
    const manifest = stored?.manifest_snapshot || boot.readSharedManifestSnapshot?.(ctx);
    if (!manifest?.layers) return null;
    if (
      !revisionsMatchManifest(
        response?.manifest_revision_digest,
        stored?.manifest_revision_digest,
      ) ||
      !revisionsMatchManifest(
        response?.surface_revision_digest,
        stored?.surface_revision_digest,
      )
    ) {
      return null;
    }
    return {
      manifest,
      layer_refs: Object.fromEntries(
        layerRefsFromManifest(manifest).map((ref) => [
          ref.name,
          { artifact_id: ref.artifact_id, content_hash: ref.content_hash },
        ]),
      ),
      compose_defaults: composeDefaultsFromResponse(response, ctx),
    };
  }

  function mergeManifestForRefetch(ctx, response) {
    const stored = boot.readViewRevision?.(ctx);
    const previous = stored?.manifest_snapshot || boot.readSharedManifestSnapshot?.(ctx) || {};
    const changedRefs = response?.assembly_plan?.layer_refs || {};
    const layers = { ...(previous.layers || {}) };
    for (const [name, ref] of Object.entries(changedRefs)) {
      layers[name] = {
        artifact_id: ref.artifact_id,
        content_hash: ref.content_hash,
        ...(ref.bytes != null ? { bytes: ref.bytes } : {}),
        ...(ref.encoding ? { encoding: ref.encoding } : {}),
      };
    }
    return {
      ...previous,
      schema_version: previous.schema_version || "mei.scene-view-manifest.v2",
      app_id: ctx.app_id || ctx.appId || previous.app_id || "",
      scene_id: ctx.scene_id || ctx.sceneId || previous.scene_id || "home",
      revision_digest: response.manifest_revision_digest,
      surface_revision_digest: response.surface_revision_digest,
      compose_defaults: response.assembly_plan?.compose_defaults || previous.compose_defaults,
      layers,
    };
  }

  async function applyViewRevision(ctx, response) {
    if (!response || response.disabled) {
      return { outcome: ViewRevisionOutcome.LOCAL_MISS, response };
    }
    const status = String(response.status || response._headers?.status || "").trim();
    if (status === ViewRevisionOutcome.ASSEMBLE_LOCAL) {
      const plan = response.assembly_plan || planFromValidatedStored(ctx, response);
      if (!plan) {
        return { outcome: ViewRevisionOutcome.LOCAL_MISS, response };
      }
      return {
        outcome: ViewRevisionOutcome.ASSEMBLE_LOCAL,
        plan,
        response,
      };
    }
    const inlined = new Set(Object.keys(response.inline_layers || {}));
    const manifest =
      response.manifest ||
      response.assembly_plan?.manifest ||
      mergeManifestForRefetch(ctx, response);
    if (response.inline_layers && boot.layerStore) {
      await storeInlineLayers(
        ctx,
        response.inline_layers,
        manifest,
      );
    }
    if (response.changed_layers?.length && boot.sceneManifestLoader?.ensureLayers) {
      const toFetch = response.changed_layers.filter((name) => !inlined.has(name));
      if (toFetch.length) {
        await boot.sceneManifestLoader.ensureLayers(
          toFetch,
          ctx.app_id,
          ctx.scene_id,
          ctx,
          manifest,
        );
      }
    }
    return {
      outcome: ViewRevisionOutcome.REFETCH,
      changed_layers: response.changed_layers || [],
      plan: {
        manifest,
        layer_refs: Object.fromEntries(
          layerRefsFromManifest(manifest).map((ref) => [
            ref.name,
            { artifact_id: ref.artifact_id, content_hash: ref.content_hash },
          ]),
        ),
        compose_defaults: composeDefaultsFromResponse(response, ctx),
      },
      response,
    };
  }

  async function negotiateViewRevision(ctx, options) {
    const response = await fetchViewRevision(ctx, options);
    return applyViewRevision(ctx, response);
  }

  function composeDefaultsForPlan(ctx, assemblyPlan) {
    return (
      assemblyPlan?.compose_defaults ||
      assemblyPlan?.manifest?.compose_defaults ||
      composeDefaultsFromResponse(assemblyPlan, ctx)
    );
  }

  function composeContextChanged(shell, ctx, assemblyPlan, options = {}) {
    if (options.forceRematerialize === true) {
      return true;
    }
    const defaults = composeDefaultsForPlan(ctx, assemblyPlan);
    const targetProjection = String(defaults?.review_projection || "").trim();
    const targetMode = String(defaults?.route_mode || ctx.surface || ctx.mode || "app")
      .trim()
      .toLowerCase();
    const targetSurface = String(ctx.surface || ctx.mode || targetMode || "app")
      .trim()
      .toLowerCase();
    const previewScroll =
      shell?.querySelector?.(".preview-pane-scroll[data-review-projection]") ||
      shell?.querySelector?.(".preview-pane-scroll");
    const currentProjection = String(
      previewScroll?.getAttribute("data-review-projection") ||
        shell?.getAttribute("data-review-projection") ||
        "",
    ).trim();
    const bodySurface = String(
      global.document?.body?.getAttribute("data-surface") ||
        global.document?.body?.getAttribute("data-mei-view") ||
        global.document?.body?.getAttribute("data-route-mode") ||
        "",
    )
      .trim()
      .toLowerCase();
    const targetApp = String(ctx.app_id || ctx.appId || "").trim().toLowerCase();
    const bodyApp = String(global.document?.body?.getAttribute("data-app-id") || "")
      .trim()
      .toLowerCase();
    const previousApp = String(options.previousApp || options.previousAppId || "")
      .trim()
      .toLowerCase();
    if (previousApp && targetApp && previousApp !== targetApp) {
      return true;
    }
    if (targetApp && bodyApp && targetApp !== bodyApp) {
      return true;
    }
    const previousSurface = String(options.previousSurface || "")
      .trim()
      .toLowerCase();
    if (previousSurface && targetSurface && previousSurface !== targetSurface) {
      return true;
    }
    if (targetSurface && bodySurface && targetSurface !== bodySurface) {
      return true;
    }
    if (targetProjection && currentProjection && targetProjection !== currentProjection) {
      return true;
    }
    if (targetMode && bodySurface && targetMode !== bodySurface) {
      return true;
    }
    return false;
  }

  async function tryAssembleLocal(ctx, plan, options = {}) {
    const assemblyPlan = plan || null;
    let layerRefs = assemblyPlan?.layer_refs || {};
    if ((!layerRefs || !Object.keys(layerRefs).length) && assemblyPlan?.manifest) {
      const refs = layerRefsFromManifest(assemblyPlan.manifest);
      layerRefs = Object.fromEntries(
        refs.map((ref) => [
          ref.name,
          { artifact_id: ref.artifact_id, content_hash: ref.content_hash },
        ]),
      );
    }
    const layers = {};
    const holdings = [];
    for (const [name, ref] of Object.entries(layerRefs)) {
      holdings.push({
        name,
        artifact_id: ref.artifact_id,
        content_hash: ref.content_hash,
      });
    }
    boot.renderPipelineMark?.("layer_restore:begin", { count: holdings.length });
    const restored = boot.layerStore?.restoreLayersByRefs
      ? await boot.layerStore.restoreLayersByRefs(
          ctx.app_id || ctx.appId,
          ctx.scene_id || ctx.sceneId,
          holdings,
        )
      : { resolved: new Map(), misses: holdings };
    for (const [name, bytes] of restored.resolved) {
      layers[name] = extractLayerDocument(bytes);
    }
    const missing = restored.misses.map((holding) => holding.name);
    if (missing.length) {
      return { ok: false, missing, layers };
    }
    const composeRoot =
      typeof boot.resolveComposeRoot === "function"
        ? boot.resolveComposeRoot(ctx.surface || ctx.mode || "app")
        : global.document?.querySelector?.(".shell, .preview-pane-scroll");
    const shell = composeRoot instanceof HTMLElement ? composeRoot : null;
    const composeAxes = {
      ...(assemblyPlan?.compose_defaults || composeDefaultsFromResponse(assemblyPlan, ctx)),
      forceRematerialize: options.forceRematerialize === true,
    };
    const forceRematerialize = composeAxes.forceRematerialize === true;
    const skipComposePreview =
      shell &&
      shell.getAttribute("data-mei-compose-placeholder") !== "1" &&
      typeof boot.previewMaterializer?.canSkipClientCompose === "function" &&
      boot.previewMaterializer.canSkipClientCompose(shell, ctx);
    if (
      shell &&
      skipComposePreview &&
      !forceRematerialize &&
      !composeContextChanged(shell, ctx, assemblyPlan, options)
    ) {
      if (typeof boot.previewMaterializer?.finalizeClientPreview === "function") {
        boot.previewMaterializer.finalizeClientPreview(shell, layers, composeAxes);
      }
      if (typeof boot.applyHostChromeFromManifestRefs === "function") {
        boot.applyHostChromeFromManifestRefs();
      }
      return { ok: true, missing: [], layers, source: "ssr_preview", materialized: true };
    }
    if (
      shell &&
      boot.previewMaterializer?.isClientLayerMaterialized?.(shell) &&
      typeof boot.hasMaterializedPreview === "function" &&
      boot.hasMaterializedPreview(shell) &&
      !forceRematerialize &&
      !composeContextChanged(shell, ctx, assemblyPlan, options)
    ) {
      if (typeof boot.applyHostChromeFromManifestRefs === "function") {
        boot.applyHostChromeFromManifestRefs();
      }
      return { ok: true, missing: [], layers, source: "ssr_preview", materialized: true };
    }
    if (boot.viewCompositor?.composeFromLayers && shell) {
      const composed = boot.viewCompositor.composeFromLayers(shell, layers, composeAxes);
      if (composed) {
        if (typeof boot.applyHostChromeFromManifestRefs === "function") {
          boot.applyHostChromeFromManifestRefs();
        }
        const materialized =
          boot.previewMaterializer?.isClientLayerMaterialized?.(shell) === true;
        return {
          ok: true,
          missing: [],
          layers,
          source: ViewRevisionOutcome.ASSEMBLE_LOCAL,
          materialized,
        };
      }
    }
    if (
      shell &&
      shell.getAttribute("data-mei-compose-placeholder") === "1" &&
      typeof boot.previewMaterializer?.materializePlaceholderPreview === "function"
    ) {
      const materialized = await boot.previewMaterializer.materializePlaceholderPreview(
        ctx,
        shell,
        layers,
        { ...options, composeAxes, forceRematerialize },
      );
      if (materialized?.ok) {
        if (typeof boot.applyHostChromeFromManifestRefs === "function") {
          boot.applyHostChromeFromManifestRefs();
        }
        const previewSource = materialized.source || "fragment";
        return {
          ok: true,
          missing: [],
          layers,
          source:
            previewSource === "compose"
              ? ViewRevisionOutcome.ASSEMBLE_LOCAL
              : "ssr_preview",
          materialized: true,
          preview_source: previewSource,
        };
      }
    }
    return { ok: false, missing: Object.keys(layerRefs), layers };
  }

  async function tryClientOnlyAssemble(ctx, options = {}) {
    const stored = boot.readViewRevision?.(ctx);
    if (!documentEnvelopeMatchesStored(ctx, stored)) {
      return null;
    }
    let manifest = stored?.manifest_snapshot || boot.readSharedManifestSnapshot?.(ctx);
    const manifestDigest =
      stored?.manifest_revision_digest || boot.readSharedManifestDigest?.(ctx) || "";
    const surfaceDigest = stored?.surface_revision_digest || "";
    if (!manifestDigest || !surfaceDigest || !manifest?.layers) {
      return null;
    }
    const layerRefs = Object.fromEntries(
      layerRefsFromManifest(manifest).map((ref) => [
        ref.name,
        { artifact_id: ref.artifact_id, content_hash: ref.content_hash },
      ]),
    );
    const plan = {
      manifest,
      layer_refs: layerRefs,
      compose_defaults: manifest.compose_defaults || composeDefaultsFromResponse({ manifest }, ctx),
    };
    const assembled = await tryAssembleLocal(ctx, plan, options);
    if (!assembled?.ok) return null;
    return {
      ...assembled,
      source: "client_cache",
    };
  }

  async function negotiateWithLocalMiss(ctx, options = {}) {
    const opts = options || {};
    const assembleOptions = {
      forceRematerialize: opts.surfaceSwitch === true || opts.forceRematerialize === true,
      previousSurface: opts.previousSurface || "",
      surfaceSwitch: opts.surfaceSwitch === true,
    };
    const cached = await tryClientOnlyAssemble(ctx, assembleOptions);
    if (cached?.ok) {
      boot.lastViewRevisionOutcome = ViewRevisionOutcome.ASSEMBLE_LOCAL;
      return {
        outcome: ViewRevisionOutcome.ASSEMBLE_LOCAL,
        assemble: cached,
        response: {
          status: ViewRevisionOutcome.ASSEMBLE_LOCAL,
          manifest_revision_digest: boot.readViewRevision?.(ctx)?.manifest_revision_digest,
          surface_revision_digest: boot.readViewRevision?.(ctx)?.surface_revision_digest,
          cached_only: true,
        },
      };
    }
    let result = await negotiateViewRevision(ctx, {
      signal: opts.signal,
      omit_digests:
        opts.surfaceSwitch === true ||
        opts.omit_digests === true ||
        (typeof boot.isSsrShellPlaceholder === "function" && boot.isSsrShellPlaceholder(ctx)),
    });
    let plan = result.plan || {
      manifest: result.response?.manifest || null,
      layer_refs: result.response?.assembly_plan?.layer_refs || {},
      compose_defaults: composeDefaultsFromResponse(result.response, ctx),
    };
    if (opts.surfaceSwitch === true) {
      plan = {
        ...plan,
        compose_defaults: buildComposeRequest(ctx),
      };
    }
    let assemble = await tryAssembleLocal(ctx, plan, assembleOptions);
    if (assemble.ok) {
      boot.lastViewRevisionOutcome =
        result.outcome === ViewRevisionOutcome.REFETCH
          ? ViewRevisionOutcome.REFETCH
          : ViewRevisionOutcome.ASSEMBLE_LOCAL;
      if (typeof boot.rememberViewRevision === "function" && result.response) {
        const rememberPayload = {
          ...result.response,
          manifest:
            result.response.manifest ||
            result.plan?.manifest ||
            boot.readSharedManifestSnapshot?.(ctx) ||
            null,
        };
        boot.rememberViewRevision(ctx, rememberPayload);
      }
      return { ...result, assemble };
    }
    const missing = assemble.missing || [];
    if (
      missing.length &&
      plan?.manifest?.layers &&
      boot.sceneManifestLoader?.ensureLayers
    ) {
      await boot.sceneManifestLoader.ensureLayers(
        missing,
        ctx.app_id || ctx.appId,
        ctx.scene_id || ctx.sceneId,
        { ...ctx, local_miss: true, signal: opts.signal },
        plan.manifest,
      );
      assemble = await tryAssembleLocal(ctx, plan, assembleOptions);
      if (assemble.ok) {
        boot.lastViewRevisionOutcome =
          result.outcome === ViewRevisionOutcome.REFETCH
            ? ViewRevisionOutcome.REFETCH
            : ViewRevisionOutcome.ASSEMBLE_LOCAL;
        return { ...result, assemble, recoveredMissing: missing };
      }
    }
    if (missing.length) {
      const detail = {
        missingCount: missing.length,
        missingSample: missing.slice(0, 12),
        priorOutcome: result.outcome || "",
        hadDigests: !(
          opts.omit_digests === true ||
          (typeof boot.isSsrShellPlaceholder === "function" && boot.isSsrShellPlaceholder(ctx))
        ),
        degraded: "missing_layers",
        recover: true,
      };
      console.warn(
        "[view-revision] missing layers after negotiate/assemble_local — contract bug; explicit recover degraded",
        detail,
      );
      if (typeof boot.renderPipelineMark === "function") {
        boot.renderPipelineMark("missing_layers", detail);
      }
      if (typeof boot.cacheDiagTrace === "function") {
        boot.cacheDiagTrace("missing-layers", detail);
      }
      result = await negotiateViewRevision(ctx, { recover: true, signal: opts.signal });
      const recoverPlan =
        result.plan || {
          manifest: result.response?.manifest || null,
          layer_refs: result.response?.assembly_plan?.layer_refs || {},
          compose_defaults: composeDefaultsFromResponse(result.response, ctx),
        };
      assemble = await tryAssembleLocal(ctx, recoverPlan, assembleOptions);
      const recoverMissing = assemble.missing || [];
      if (
        recoverMissing.length &&
        recoverPlan?.manifest?.layers &&
        boot.sceneManifestLoader?.ensureLayers
      ) {
        await boot.sceneManifestLoader.ensureLayers(
          recoverMissing,
          ctx.app_id || ctx.appId,
          ctx.scene_id || ctx.sceneId,
          { ...ctx, local_miss: true, signal: opts.signal },
          recoverPlan.manifest,
        );
        assemble = await tryAssembleLocal(ctx, recoverPlan, assembleOptions);
      }
      if (assemble.ok) {
        boot.lastViewRevisionOutcome = ViewRevisionOutcome.REFETCH;
        if (typeof boot.rememberViewRevision === "function" && result.response) {
          const rememberPayload = {
            ...result.response,
            manifest:
              result.response.manifest ||
              result.plan?.manifest ||
              boot.readSharedManifestSnapshot?.(ctx) ||
              null,
          };
          boot.rememberViewRevision(ctx, rememberPayload);
        }
        return { ...result, assemble, degraded: "missing_layers" };
      }
    }
    boot.lastViewRevisionOutcome = ViewRevisionOutcome.LOCAL_MISS;
    return { ...result, assemble, outcome: ViewRevisionOutcome.LOCAL_MISS };
  }

  boot.ViewRevisionOutcome = ViewRevisionOutcome;
  boot.holdingsFromLayerCache = holdingsFromLayerCache;
  boot.revisionsMatchManifest = revisionsMatchManifest;
  boot.viewRevisionClient = {
    isEnabled: isViewRevisionEnabled,
    fetchViewRevision,
    applyViewRevision,
    negotiateViewRevision,
    negotiateWithLocalMiss,
    negotiateViewRevisionWithRecover: negotiateWithLocalMiss,
    tryAssembleLocal,
    tryClientOnlyAssemble,
    layerRefsFromManifest,
    buildComposeRequest,
  };
})(typeof window !== "undefined" ? window : globalThis);
