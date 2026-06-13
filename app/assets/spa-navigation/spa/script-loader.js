  function loadScript(rawSrc, options) {
    const opts = options || {};
    const absolute = new URL(rawSrc, window.location.href).toString();
    if (opts.persistentKey) {
      const found = document.querySelector(
        'script[data-mei-persistent-script="' + opts.persistentKey + '"]',
      );
      if (found) return Promise.resolve();
    }
    if (opts.reloadKey) {
      document
        .querySelectorAll('script[data-mei-reload-script="' + opts.reloadKey + '"]')
        .forEach((node) => node.remove());
    }
    return new Promise((resolve, reject) => {
      let settled = false;
      const finish = (fn) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        fn();
      };
      const timer = setTimeout(() => {
        if (opts.softFail) {
          console.warn("[spa-navigation] script load timeout", rawSrc);
          finish(resolve);
          return;
        }
        finish(() => reject(new Error("script load timeout: " + rawSrc)));
      }, SCRIPT_LOAD_TIMEOUT_MS);
      const script = document.createElement("script");
      if (opts.module) script.type = "module";
      script.src = absolute;
      script.async = false;
      if (opts.persistentKey) {
        script.setAttribute("data-mei-persistent-script", opts.persistentKey);
      }
      if (opts.sceneBundle) {
        script.setAttribute("data-mei-scene-bundle", "true");
      }
      if (opts.reloadKey) {
        script.setAttribute("data-mei-reload-script", opts.reloadKey);
      }
      script.onload = () => finish(resolve);
      script.onerror = () => {
        if (opts.softFail) {
          console.warn("[spa-navigation] script load skipped", rawSrc);
          finish(resolve);
          return;
        }
        finish(() => reject(new Error("failed to load script: " + rawSrc)));
      };
      document.body.appendChild(script);
    });
  }

