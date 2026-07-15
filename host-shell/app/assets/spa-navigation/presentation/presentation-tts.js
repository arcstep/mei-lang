(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});

  const state = {
    enabled: false,
    autoSpeak: false,
    preferSpeakerNotes: false,
    speaking: false,
    utterance: null,
  };

  function speechApi() {
    return typeof window !== "undefined" ? window.speechSynthesis : null;
  }

  function isSupported() {
    return Boolean(speechApi() && typeof window.SpeechSynthesisUtterance === "function");
  }

  function stripHtml(value) {
    const node = document.createElement("div");
    node.innerHTML = String(value || "");
    return String(node.textContent || "").replace(/\s+/g, " ").trim();
  }

  function resolveStepText(step, options = {}) {
    if (!step || typeof step !== "object") return "";
    const preferNotes =
      options.preferSpeakerNotes === true || state.preferSpeakerNotes === true;
    if (preferNotes) {
      const notes = stripHtml(step.speakerNotesHtml || step.speaker_notes || step.notes || "");
      if (notes) return notes;
    }
    const caption = stripHtml(step.captionHtml || step.caption || step.title || "");
    if (caption) return caption;
    if (!preferNotes) {
      return stripHtml(step.speakerNotesHtml || step.speaker_notes || step.notes || "");
    }
    return "";
  }

  function stopSpeech() {
    const api = speechApi();
    if (!api) return;
    api.cancel();
    state.speaking = false;
    state.utterance = null;
  }

  function speakText(text, options = {}) {
    const api = speechApi();
    if (!api || !isSupported()) return false;
    const normalized = String(text || "").replace(/\s+/g, " ").trim();
    if (!normalized) return false;
    stopSpeech();
    const utterance = new window.SpeechSynthesisUtterance(normalized);
    utterance.lang = String(options.lang || "zh-CN").trim() || "zh-CN";
    utterance.rate = Number.isFinite(Number(options.rate)) ? Number(options.rate) : 1;
    utterance.onstart = () => {
      state.speaking = true;
    };
    utterance.onend = () => {
      state.speaking = false;
      state.utterance = null;
    };
    utterance.onerror = () => {
      state.speaking = false;
      state.utterance = null;
    };
    state.utterance = utterance;
    api.speak(utterance);
    return true;
  }

  function speakStep(step, options = {}) {
    if (!state.enabled && options.force !== true) return false;
    return speakText(resolveStepText(step, options), options);
  }

  function setEnabled(next) {
    state.enabled = Boolean(next);
    if (!state.enabled) {
      stopSpeech();
    }
    return state.enabled;
  }

  function toggleEnabled() {
    return setEnabled(!state.enabled);
  }

  function setAutoSpeak(next) {
    state.autoSpeak = Boolean(next);
    return state.autoSpeak;
  }

  function setPreferSpeakerNotes(next) {
    state.preferSpeakerNotes = Boolean(next);
    return state.preferSpeakerNotes;
  }

  const tts = {
    isSupported,
    speakStep,
    speakText,
    stopSpeech,
    setEnabled,
    toggleEnabled,
    setAutoSpeak,
    setPreferSpeakerNotes,
    resolveStepText,
    get state() {
      return { ...state };
    },
  };

  boot.presentationTts = tts;
})();
