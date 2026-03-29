(function () {
  const SESSION_PROVIDER_ID = "session_ephemeral";

  function clampInteger(value, fallback, min, max) {
    const number = Number(value);
    if (!Number.isFinite(number)) return fallback;
    return Math.min(max, Math.max(min, Math.round(number)));
  }

  function normalizeMemorySettings(settings) {
    const source = settings || {};
    return {
      enabled: source.enabled !== false,
      provider: String(source.provider || SESSION_PROVIDER_ID).trim() || SESSION_PROVIDER_ID,
      maxItems: clampInteger(source.maxItems, 12, 1, 64),
      ttlHours: clampInteger(source.ttlHours, 24, 1, 24),
    };
  }

  function buildFact(value, kind, keyPrefix, content) {
    const normalized = String(value || "").trim();
    if (!normalized) return null;
    return {
      kind,
      key: `${keyPrefix}:${normalized}`,
      content,
    };
  }

  function extractFactsFromUserText(text) {
    const source = String(text || "").trim();
    if (!source) return [];

    const matchers = [
      {
        regex: /(?:我叫|我的名字是|你可以叫我|以后叫我)\s*([^\s，。！？；,.!?]{1,16})/,
        build: (match) => buildFact(match?.[1], "identity", "name", `用户希望被称呼为“${match[1].trim()}”`),
      },
      {
        regex: /(?:我喜欢|我比较喜欢|我更喜欢)\s*([^，。！？；,.!?]{1,24})/,
        build: (match) => buildFact(match?.[1], "preference", "like", `用户喜欢${match[1].trim()}`),
      },
      {
        regex: /(?:我不喜欢|我讨厌|我不太喜欢)\s*([^，。！？；,.!?]{1,24})/,
        build: (match) => buildFact(match?.[1], "preference", "dislike", `用户不喜欢${match[1].trim()}`),
      },
      {
        regex: /(?:我最近在|我现在在|这段时间在)\s*([^，。！？；,.!?]{2,28})/,
        build: (match) => buildFact(match?.[1], "context", "recent", `用户最近在${match[1].trim()}`),
      },
      {
        regex: /(?:我是|我是一个|我是个)\s*([^，。！？；,.!?]{2,24})/,
        build: (match) => buildFact(match?.[1], "profile", "profile", `用户是${match[1].trim()}`),
      },
      {
        regex: /(?:我在做|我正在做|我想做|我准备)\s*([^，。！？；,.!?]{2,28})/,
        build: (match) => buildFact(match?.[1], "task", "task", `用户当前在做${match[1].trim()}`),
      },
    ];

    const facts = [];
    for (const matcher of matchers) {
      const match = source.match(matcher.regex);
      const fact = matcher.build(match);
      if (fact) facts.push(fact);
    }
    return facts;
  }

  class SessionEphemeralMemoryProvider {
    constructor(settings) {
      this.configure(settings);
      this.reset();
    }

    configure(settings) {
      this.settings = normalizeMemorySettings(settings);
    }

    reset() {
      this.startedAt = Date.now();
      this.items = [];
    }

    prune(now = Date.now()) {
      const sessionDeadline = this.startedAt + this.settings.ttlHours * 60 * 60 * 1000;
      const cutoff = Math.min(now, sessionDeadline);
      this.items = this.items.filter((item) => item.expiresAt > cutoff);
      if (this.items.length > this.settings.maxItems) {
        this.items = this.items
          .slice()
          .sort((left, right) => right.updatedAt - left.updatedAt)
          .slice(0, this.settings.maxItems);
      }
    }

    rememberTurn(userText) {
      if (!this.settings.enabled) return;

      const now = Date.now();
      const expiresAt = Math.min(
        this.startedAt + this.settings.ttlHours * 60 * 60 * 1000,
        now + this.settings.ttlHours * 60 * 60 * 1000,
      );

      for (const fact of extractFactsFromUserText(userText)) {
        const existing = this.items.find((item) => item.key === fact.key);
        if (existing) {
          existing.content = fact.content;
          existing.kind = fact.kind;
          existing.updatedAt = now;
          existing.expiresAt = expiresAt;
          continue;
        }
        this.items.push({
          ...fact,
          createdAt: now,
          updatedAt: now,
          expiresAt,
        });
      }

      this.prune(now);
    }

    buildPrompt() {
      if (!this.settings.enabled) return "";

      this.prune();
      if (!this.items.length) return "";

      const lines = this.items
        .slice()
        .sort((left, right) => right.updatedAt - left.updatedAt)
        .slice(0, this.settings.maxItems)
        .map((item) => `- ${item.content}`);

      return [
        "以下是当前会话中的临时记忆，只在本次启动内有效：",
        ...lines,
        "仅在与当前用户问题相关时自然使用，不要机械复述这些记忆。",
      ].join("\n");
    }
  }

  function createMemoryProvider(settings) {
    const normalized = normalizeMemorySettings(settings);
    switch (normalized.provider) {
      case SESSION_PROVIDER_ID:
      default:
        return new SessionEphemeralMemoryProvider(normalized);
    }
  }

  window.FastAvatarMemory = {
    SESSION_PROVIDER_ID,
    createMemoryProvider,
    normalizeMemorySettings,
  };
})();
