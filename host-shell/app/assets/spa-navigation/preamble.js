(() => {
  const global = typeof window !== "undefined" ? window : globalThis;
  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});
  if (boot.spaNavigationMounted) return;
  boot.spaNavigationMounted = true;

