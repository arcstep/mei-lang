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

  async function listClientHoldings(ctx) {
    if (boot.layerStore?.listHoldings) {
      return boot.layerStore.listHoldings(ctx.app_id, ctx.scene_id);
    }
    if (boot.layerArtifactCache?.listHoldings) {
      return boot.layerArtifactCache.listHoldings(ctx.app_id, ctx.scene_id);
    }
    return [];
  }

  function buildComposeRequest(ctx) {
    const surface = String(ctx.surface || ctx.mode || "app")
      .trim()
      .toLowerCase();
    const tab =
      String(ctx.tab || "").trim() || (surface === "build" ? "preview" : "scene");
    return {
      route_mode: surface,
      tab,
      chrome: ctx.chrome || "",
      review_projection: ctx.review_projection || ctx.reviewProjection || "",
      data_mode: ctx.data_mode || ctx.dataMode || "",
      focus: ctx.focus || "",
      scope: ctx.scope || "",
    };
  }

  function composeDefaultsFromResponse(response, ctx) {
    return (
      response?.compose_defaults ||
      response?.manifest?.compose_defaults ||
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
      surface: ctx.surface || ctx.mode || "app",
    });
    const compose = buildComposeRequest(ctx);
    params.set("compose", JSON.stringify(compose));
    if (ctx.node) params.set("node", ctx.node);
    if (opts.local_miss) {
      params.set("local_miss", "1");
      if (opts.missing_layers?.length) {
        params.set("missing_layers", opts.missing_layers.join(","));
      }
    }
    const holdings = opts.client_layers || (await listClientHoldings(ctx));
    if (holdings.length) {
      params.set("client_layers", JSON.stringify(holdingsFromLayerCache(holdings)));
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

  async function storeInlineLayers(ctx, inlineLayers, manifest) {
    if (!inlineLayers || !boot.layerStore) return;
    for (const [name, bytes] of Object.entries(inlineLayers)) {
      const ref = layerRefFromManifestValue(name, manifest?.layers?.[name]);
      if (!ref) continue;
      await boot.layerStore.putLayerByRef(ctx.app_id, ctx.scene_id, ref, bytes, manifest);
    }
  }

  async function applyViewRevision(ctx, response) {
    if (!response || response.disabled) {
      return { outcome: ViewRevisionOutcome.LOCAL_MISS, response };
    }
    const status = String(response.status || response._headers?.status || "").trim();
    if (status === ViewRevisionOutcome.ASSEMBLE_LOCAL && response.assembly_plan) {
      return {
        outcome: ViewRevisionOutcome.ASSEMBLE_LOCAL,
        plan: response.assembly_plan,
        response,
      };
    }
    if (response.inline_layers && boot.layerStore) {
      await storeInlineLayers(
        ctx,
        response.inline_layers,
        response.manifest || response.assembly_plan?.manifest || null,
      );
    }
    if (response.changed_layers?.length && boot.sceneManifestLoader?.ensureLayers) {
      const manifest = response.manifest || response.assembly_plan?.manifest;
      await boot.sceneManifestLoader.ensureLayers(
        response.changed_layers,
        ctx.app_id,
        ctx.scene_id,
        ctx,
        manifest,
      );
    }
    return {
      outcome: ViewRevisionOutcome.REFETCH,
      changed_layers: response.changed_layers || [],
      response,
    };
  }

  async function negotiateViewRevision(ctx, options) {
    const response = await fetchViewRevision(ctx, options);
    return applyViewRevision(ctx, response);
  }

  async function tryAssembleLocal(ctx, plan) {
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
    const missing = [];
    const layers = {};
    for (const [name, ref] of Object.entries(layerRefs)) {
      const holding = {
        name,
        artifact_id: ref.artifact_id,
        content_hash: ref.content_hash,
      };
      let bytes = boot.layerStore?.takeLayerByRef?.(holding);
      if (!bytes && boot.layerArtifactCache) {
        const cached = await boot.layerArtifactCache.getLayer(ref.artifact_id);
        if (
          cached &&
          cached.content_hash === ref.content_hash &&
          cached.bytes != null
        ) {
          bytes = cached.bytes;
          if (boot.layerStore?.putLayerByRef) {
            boot.layerStore.putLayerByRef(ctx.app_id, ctx.scene_id, holding, bytes, assemblyPlan.manifest);
          }
        }
      }
      if (!bytes) {
        missing.push(name);
        continue;
      }
      layers[name] = bytes;
    }
    if (missing.length) {
      return { ok: false, missing, layers };
    }
    const composeRoot =
      typeof boot.resolveComposeRoot === "function"
        ? boot.resolveComposeRoot(ctx.surface || ctx.mode || "app")
        : global.document?.querySelector?.(".shell, .preview-pane-scroll");
    const shell = composeRoot instanceof HTMLElement ? composeRoot : null;
    if (
      shell &&
      typeof boot.hasMaterializedPreview === "function" &&
      boot.hasMaterializedPreview(shell)
    ) {
      if (typeof boot.applyHostChromeFromManifestRefs === "function") {
        boot.applyHostChromeFromManifestRefs();
      }
      return { ok: true, missing: [], layers, source: "ssr_preview" };
    }
    if (boot.viewCompositor?.composeFromLayers && shell) {
      const composed = boot.viewCompositor.composeFromLayers(
        shell,
        layers,
        assemblyPlan?.compose_defaults || composeDefaultsFromResponse(assemblyPlan, ctx),
      );
      if (composed) {
        if (typeof boot.applyHostChromeFromManifestRefs === "function") {
          boot.applyHostChromeFromManifestRefs();
        }
        return { ok: true, missing: [], layers, source: ViewRevisionOutcome.ASSEMBLE_LOCAL };
      }
    }
    return { ok: false, missing: Object.keys(layerRefs), layers };
  }

  async function negotiateWithLocalMiss(ctx) {
    let result = await negotiateViewRevision(ctx, {});
    let assemble = await tryAssembleLocal(
      ctx,
      result.plan || {
        manifest: result.response?.manifest || null,
        compose_defaults: composeDefaultsFromResponse(result.response, ctx),
      },
    );
    if (assemble.ok) {
      boot.lastViewRevisionOutcome =
        result.outcome === ViewRevisionOutcome.REFETCH
          ? ViewRevisionOutcome.REFETCH
          : ViewRevisionOutcome.ASSEMBLE_LOCAL;
      return { ...result, assemble };
    }
    result = await negotiateViewRevision(ctx, {
      local_miss: true,
      missing_layers: assemble.missing,
    });
    assemble = await tryAssembleLocal(
      ctx,
      result.plan || {
        manifest: result.response?.manifest || null,
        compose_defaults: composeDefaultsFromResponse(result.response, ctx),
      },
    );
    if (assemble.ok) {
      boot.lastViewRevisionOutcome = ViewRevisionOutcome.REFETCH;
      return { ...result, assemble };
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
    tryAssembleLocal,
    layerRefsFromManifest,
  };
})(typeof window !== "undefined" ? window : globalThis);
