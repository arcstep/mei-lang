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

  function defaultReviewProjectionForSurface(surface) {
    const slug = String(surface || "app").trim().toLowerCase();
    if (slug === "layout") return "plane_region_section";
    if (slug === "prototype") return "static_full";
    return "live_full";
  }

  function defaultDataModeForSurface(surface) {
    const slug = String(surface || "app").trim().toLowerCase();
    if (slug === "layout" || slug === "prototype") return "static";
    return "eval";
  }

  function buildComposeRequest(ctx) {
    const resolveSurface =
      boot.sceneManifestLoader?.resolveWorkspaceSurface ||
      ((value) => String(value || "app").trim().toLowerCase() || "app");
    const defaultTab =
      boot.sceneManifestLoader?.defaultTabForSurface ||
      ((surface) => (surface === "layout" || surface === "prototype" ? "preview" : "scene"));
    const surface = resolveSurface(ctx.surface || ctx.mode || "app");
    const tab = String(ctx.tab || "").trim() || defaultTab(surface);
    const reviewFromCtx = String(ctx.review_projection || ctx.reviewProjection || "").trim();
    const dataFromCtx = String(ctx.data_mode || ctx.dataMode || "").trim();
    return {
      route_mode: surface,
      tab,
      chrome: String(ctx.chrome || "").trim(),
      review_projection: reviewFromCtx || defaultReviewProjectionForSurface(surface),
      data_mode: dataFromCtx || defaultDataModeForSurface(surface),
      focus: String(ctx.focus || "").trim(),
      scope: String(ctx.scope || "").trim(),
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
    const resolveSurface =
      boot.sceneManifestLoader?.resolveWorkspaceSurface ||
      ((value) => String(value || "app").trim().toLowerCase() || "app");
    const params = new URLSearchParams({
      app_id: ctx.app_id || ctx.appId || "",
      scene: ctx.scene_id || ctx.sceneId || "home",
      surface: resolveSurface(ctx.surface || ctx.mode || "app"),
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
    for (const [name, bytes] of Object.entries(inlineLayers)) {
      const ref = layerRefFromManifestValue(name, manifest?.layers?.[name]);
      if (!ref) continue;
      const document = extractLayerDocument(bytes);
      await boot.layerStore.putLayerByRef(ctx.app_id, ctx.scene_id, ref, document, manifest);
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
    const inlined = new Set(Object.keys(response.inline_layers || {}));
    if (response.inline_layers && boot.layerStore) {
      await storeInlineLayers(
        ctx,
        response.inline_layers,
        response.manifest || response.assembly_plan?.manifest || null,
      );
    }
    if (response.changed_layers?.length && boot.sceneManifestLoader?.ensureLayers) {
      const toFetch = response.changed_layers.filter((name) => !inlined.has(name));
      if (toFetch.length) {
        const manifest = response.manifest || response.assembly_plan?.manifest;
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
    const missing = [];
    const layers = {};
    for (const [name, ref] of Object.entries(layerRefs)) {
      const holding = {
        name,
        artifact_id: ref.artifact_id,
        content_hash: ref.content_hash,
      };
      let bytes = boot.layerStore?.takeLayerByRef?.(holding);
      if (!bytes && boot.sceneManifestLoader?.resolveLayerBytes) {
        bytes = await boot.sceneManifestLoader.resolveLayerBytes(
          holding,
          ctx.app_id || ctx.appId,
          ctx.scene_id || ctx.sceneId,
          assemblyPlan?.manifest,
        );
      } else if (!bytes && boot.layerArtifactCache) {
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
      layers[name] = extractLayerDocument(bytes);
    }
    if (missing.length) {
      return { ok: false, missing, layers };
    }
    const composeRoot =
      typeof boot.resolveComposeRoot === "function"
        ? boot.resolveComposeRoot(ctx.surface || ctx.mode || "app")
        : global.document?.querySelector?.(".shell, .preview-pane-scroll");
    const shell = composeRoot instanceof HTMLElement ? composeRoot : null;
    const ssrPreviewReady =
      shell &&
      boot.previewMaterializer?.isSsrInjectedPreviewRoot?.(shell) === true;
    const forceRematerialize = options.forceRematerialize === true;
    if (
      shell &&
      ssrPreviewReady &&
      !forceRematerialize &&
      !composeContextChanged(shell, ctx, assemblyPlan, options)
    ) {
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
      const composeAxes = {
        ...(assemblyPlan?.compose_defaults || composeDefaultsFromResponse(assemblyPlan, ctx)),
        forceRematerialize,
      };
      const composed = boot.viewCompositor.composeFromLayers(shell, layers, composeAxes);
      if (composed) {
        if (typeof boot.applyHostChromeFromManifestRefs === "function") {
          boot.applyHostChromeFromManifestRefs();
        }
        const materialized =
          typeof boot.hasMaterializedPreview === "function" && boot.hasMaterializedPreview(shell);
        return {
          ok: true,
          missing: [],
          layers,
          source: ViewRevisionOutcome.ASSEMBLE_LOCAL,
          materialized,
        };
      }
    }
    return { ok: false, missing: Object.keys(layerRefs), layers };
  }

  async function tryClientOnlyAssemble(ctx, options = {}) {
    const stored = boot.readViewRevision?.(ctx);
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
    };
    if (!opts.surfaceSwitch) {
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
    }
    let result = await negotiateViewRevision(ctx, {
      signal: opts.signal,
      omit_digests: opts.surfaceSwitch === true,
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
    if (missing.length) {
      result = await negotiateViewRevision(ctx, { recover: true, signal: opts.signal });
      assemble = await tryAssembleLocal(
        ctx,
        result.plan || {
          manifest: result.response?.manifest || null,
          layer_refs: result.response?.assembly_plan?.layer_refs || {},
          compose_defaults: composeDefaultsFromResponse(result.response, ctx),
        },
        assembleOptions,
      );
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
        return { ...result, assemble };
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
  };
})(typeof window !== "undefined" ? window : globalThis);
