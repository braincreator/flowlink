import { CopyButton } from "./components";

export default function Home() {
  return (
    <div>
      {/* ═══ NAV — glassmorphism, sticky ═══ */}
      <nav className="container">
        <span className="logo">
          ⚡ flow<span>link</span>
        </span>
        <div className="nav-links">
          <a href="#how">Как работает</a>
          <a href="#features">Возможности</a>
          <a href="#pricing">Цены</a>
          <a href="#faq">FAQ</a>
          <a href="/playground">Демо 🛡️</a>
        </div>
      </nav>

      {/* ═══ HERO ═══ */}
      <section className="hero container">
        <div className="hero-badge">
          <span className="dot"></span>
          AI Security Shield · E2EE
        </div>

        <h1>
          <span className="accent">Защита серверов</span> с{" "}
          <span className="command">AI-агентами</span>
        </h1>

        <p className="hero-sub">
          Перехватывает, анализирует и блокирует опасные команды на kernel-level. E2EE, GitOps rollback, K8s operator.
        </p>

        <div className="hero-buttons">
          <a href="/playground" className="btn btn-primary">
            Попробуй демо 🛡️
          </a>
          <a href="#how" className="btn btn-secondary">
            Как это работает
          </a>
        </div>

        {/* ── Remotion terminal animation ── */}
        <div className="video-hero">
          <video
            autoPlay
            loop
            muted
            playsInline
            poster="/hero-terminal-poster.png"
          >
            <source src="/hero-terminal.mp4" type="video/mp4" />
          </video>
        </div>
      </section>

      {/* ═══ CHAPTER 1: PROBLEM → SOLUTION ═══ */}
      <section className="problem-section container">
        <h2>AI-агенты — новая угроза</h2>
        <p className="section-sub">
          Каждый день AI-агенты ломают продакшен
        </p>

        <div className="problems-grid">
          <div className="problem-card">
            <span className="emoji">💀</span>
            <h3>AI удалил продакшен базу</h3>
            <p>
              Claude Code выполнил rm -rf /app/data. Без подтверждения, без бэкапа. Сервер мёртв.
            </p>
          </div>
          <div className="problem-card">
            <span className="emoji">⏰</span>
            <h3>Обфускация прошла мимо</h3>
            <p>
              Команда cmd=$(echo cm0gLXJm | base64 -d); $cmd не распознаётся базовым pattern matching.
            </p>
          </div>
          <div className="problem-card">
            <span className="emoji">🧠</span>
            <h3>Кто что сделал?</h3>
            <p>
              5 AI-агентов на одном сервере. DROP TABLE, chmod 777, docker rm — кто виноват?
            </p>
          </div>
        </div>

        <div className="solution-box">
          <h3>FlowLink = AI Security Shield</h3>
          <p>
            Kernel-level перехват опасных команд. AST-анализ обфускации. Auto-бэкап перед каждой угрозой. Восстановление за секунды.
          </p>
        </div>
      </section>

      {/* ═══ CHAPTER 2: HOW IT WORKS ═══ */}
      <section className="container" id="how">
        <h2>Как это работает</h2>
        <p className="section-sub">Подключи сервер — и он под защитой</p>

        <div className="steps">
          <div className="step">
            <div className="step-number">1</div>
            <h3>Зарегистрируйся</h3>
            <p>Создай аккаунт — бесплатно, без карты</p>
          </div>
          <div className="step">
            <div className="step-number">2</div>
            <h3>Подключи сервер</h3>
            <p>
              Установи агента одной командой через дашборд. Ubuntu, Debian, CentOS, Arch.
            </p>
          </div>
          <div className="step">
            <div className="step-number">3</div>
            <h3>Опасное = заблокировано</h3>
            <p>
              Risk score 0-10. Авто-бэкап перед угрозой. Approval для ambiguous.
            </p>
          </div>
        </div>
      </section>

      {/* ═══ FEATURES — Bento Grid ═══ */}
      <section className="container" id="features">
        <h2>Возможности</h2>
        <p className="section-sub">
          Всё что нужно для защиты серверов с AI-агентами
        </p>

        <div className="features-grid">
          <div className="feature-card">
            <span className="icon">⏪</span>
            <h3>Shield</h3>
            <p>
              Kernel-level перехват. 50+ паттернов, AST-анализ, eBPF syscall interception. Risk score 0-10.
            </p>
            <span className="tag tag-amber">Security</span>
          </div>

          <div className="feature-card">
            <span className="icon">🔄</span>
            <h3>Smart Backup</h3>
            <p>
              Auto-бэкап перед каждой опасной командой. Diff-based, dedup. Килобайты вместо гигабайт.
            </p>
            <span className="tag tag-green">Core</span>
          </div>

          <div className="feature-card">
            <span className="icon">🔐</span>
            <h3>E2EE</h3>
            <p>
              X25519 + AES-256-GCM. Приватные ключи только на агенте. Relay не может расшифровать.
            </p>
            <span className="tag tag-amber">Security</span>
          </div>

          <div className="feature-card">
            <span className="icon">📊</span>
            <h3>GitOps</h3>
            <p>
              Auto-rollback при config drift. Semantic diff. Circuit breaker для каскадных сбоев.
            </p>
            <span className="tag tag-blue">Infrastructure</span>
          </div>

          <div className="feature-card">
            <span className="icon">☸️</span>
            <h3>K8s Operator</h3>
            <p>
              CRD FlowLinkShieldPolicy, sidecar injection, admission webhook. Нативная интеграция.
            </p>
            <span className="tag tag-blue">Infrastructure</span>
          </div>

          <div className="feature-card">
            <span className="icon">📋</span>
            <h3>Audit</h3>
            <p>
              HMAC-верифицированный лог. 1-90 дней retention. SIEM export (CEF/LEEF/JSON).
            </p>
            <span className="tag tag-blue">Compliance</span>
          </div>
        </div>
      </section>

      {/* ═══ WHO IS IT FOR ═══ */}
      <section className="container">
        <h2>Кому нужен FlowLink</h2>
        <p className="section-sub">
          Если на серверах работают AI-агенты — тебе это нужно
        </p>

        <div className="audience-grid">
          <div className="audience-card">
            <span className="emoji">👨‍💻</span>
            <h3>Девелоперы</h3>
            <p>Фрилансеры с Claude Code, Codex, GPT на VPS. 1-3 сервера.</p>
          </div>
          <div className="audience-card">
            <span className="emoji">🔧</span>
            <h3>DevOps-команды</h3>
            <p>Стандарт, аудит, approval workflow. Multi-server управление.</p>
          </div>
          <div className="audience-card">
            <span className="emoji">🤖</span>
            <h3>AI-боты</h3>
            <p>Автономные AI-агенты на продакшене. Protection от саморазрушения.</p>
          </div>
          <div className="audience-card">
            <span className="emoji">🏢</span>
            <h3>SaaS-стартапы</h3>
            <p>Production safety, compliance, SLA. K8s + GitOps.</p>
          </div>
        </div>
      </section>

      {/* ═══ PRICING ═══ */}
      <section className="container" id="pricing">
        <h2>Тарифы</h2>
        <p className="section-sub">Платишь за масштаб инфраструктуры — а не за запросы</p>

        <div className="pricing-grid">
          <div className="pricing-card">
            <h3>Trial</h3>
            <div className="price">0 ₽</div>
            <div className="price-note">7 дней</div>
            <ul>
              <li>1 хост</li>
              <li>1 юзер</li>
              <li>3 дня логов</li>
              <li>Pattern blocking</li>
              <li>Manual backup</li>
              <li>E2EE</li>
            </ul>
          </div>

          <div className="pricing-card">
            <h3>Starter</h3>
            <div className="price">2 990 ₽</div>
            <div className="price-note">/мес</div>
            <p className="price-yearly">23 920 ₽ /год (-33%)</p>
            <ul>
              <li>3 хоста</li>
              <li>3 юзера</li>
              <li>14 дней логов</li>
              <li>AST-анализ</li>
              <li>Canary honeypots</li>
              <li>Approval workflow</li>
              <li>Custom policies (до 10)</li>
              <li>Smart backup + dedup</li>
              <li>Device trust</li>
              <li>MCP protocol</li>
            </ul>
          </div>

          <div className="pricing-card featured">
            <h3>Pro</h3>
            <div className="price">7 990 ₽</div>
            <div className="price-note">/мес</div>
            <p className="price-yearly">63 920 ₽ /год (-33%)</p>
            <ul>
              <li>20 хостов</li>
              <li>10 юзеров</li>
              <li>90 дней логов</li>
              <li>eBPF kernel-level</li>
              <li>Policy DSL</li>
              <li>Forensics</li>
              <li>K8s operator</li>
              <li>GitOps</li>
              <li>SIEM export</li>
              <li>RBAC</li>
              <li>Telegram approval</li>
              <li>Auto restore</li>
              <li>LLM failover</li>
              <li>Global kill switch</li>
            </ul>
          </div>
        </div>

        <p className="pricing-enterprise">
          Больше 20 хостов? <a href="mailto:hello@flowlink.app">Свяжитесь с нами</a>
        </p>
      </section>

      {/* ═══ FAQ ═══ */}
      <section className="container" id="faq">
        <h2>FAQ</h2>
        <p className="section-sub">Частые вопросы</p>

        <div className="faq-list">
          <div className="faq-item">
            <h3>Зачем FlowLink, если есть Falco / OPA / sudo?</h3>
            <p>
              Falco = runtime alerting (не блокирует). OPA = policy как код (сложно). sudo = insufficient для AI. FlowLink = AI-native: понимает контекст, блокирует на kernel-level, auto-бэкапит.
            </p>
          </div>

          <div className="faq-item">
            <h3>Насколько это безопасно?</h3>
            <p>
              E2EE шифрование X25519 + AES-256-GCM. Приватные ключи хранятся
              только на агенте — relay-сервер не может расшифровать данные. Все
              команды проходят через Policy Layer с 50+ blacklist-паттернами,
              sandbox и approval.
            </p>
          </div>

          <div className="faq-item">
            <h3>Сколько ресурсов жрёт агент?</h3>
            <p>
              Rust binary: ~15MB RAM idle, ~50MB при пике. CPU: &lt;1% при мониторинге. E2EE: 0 overhead на relay.
            </p>
          </div>

          <div className="faq-item">
            <h3>Что если агент упадёт?</h3>
            <p>
              Локальные бэкапы хранятся на сервере. Даже если relay недоступен, все бэкапы на месте. Восстановление через дашборд или вручную.
            </p>
          </div>

          <div className="faq-item">
            <h3>Какие команды блокируются?</h3>
            <p>
              50+ паттернов в 4 категориях + AST-анализ обфускации + eBPF syscall interception. Starter: pattern + AST. Pro: + eBPF kernel-level.
            </p>
          </div>

          <div className="faq-item">
            <h3>Сколько агентов можно подключить?</h3>
            <p>
              Без ограничений. FlowLink защищает хосты, а не агенты. Сколько угодно AI-агентов на одном или нескольких хостах — тариф зависит только от количества серверов и глубины защиты.
            </p>
          </div>

          <div className="faq-item">
            <h3>Как trial работает?</h3>
            <p>
              7 дней бесплатно: 1 хост, pattern blocking, E2EE. Без карты. После trial — переход на Starter или Free (ограниченный просмотр логов).
            </p>
          </div>
        </div>
      </section>

      {/* ═══ FOOTER ═══ */}
      <footer className="container">
        <p>
          <a href="/playground">Демо</a>{" · "}
          <a href="#pricing">Тарифы</a>{" · "}
          <a href="/privacy">Конфиденциальность</a>{" · "}
          <a href="/terms">Условия</a>{" · "}
          FlowMasters © 2026
        </p>
      </footer>
    </div>
  );
}
