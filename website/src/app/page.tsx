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
          <a
            href="https://github.com/braincreator/flowlink"
            target="_blank"
            rel="noopener noreferrer"
          >
            GitHub ↗
          </a>
        </div>
      </nav>

      {/* ═══ HERO ═══ */}
      <section className="hero container">
        <div className="hero-badge">
          <span className="dot"></span>
          AI-native · Open Source · E2EE
        </div>

        <h1>
          <span className="accent">Ctrl+Z</span> для{" "}
          <span className="command">rm -rf</span>
        </h1>

        <p className="hero-sub">
          AI автоматически бекапит только то, что может сломаться — и
          восстанавливает за секунды. Поставил — забыл.
        </p>

        <div className="hero-buttons">
          <a href="#install" className="btn btn-primary">
            Установить бесплатно →
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
        <h2>Знакомо?</h2>
        <p className="section-sub">
          Каждый девелопер хотя бы раз ломал сервер командой
        </p>

        <div className="problems-grid">
          <div className="problem-card">
            <span className="emoji">💀</span>
            <h3>rm -rf без бекапа</h3>
            <p>
              Удалил конфиг, базу, логи — и паника. Полный бекап делать долго,
              а надо было вчера.
            </p>
          </div>
          <div className="problem-card">
            <span className="emoji">⏰</span>
            <h3>Часы на восстановление</h3>
            <p>
              Полный бекап VPS — 10-100 GB. Восстановление = даунтайм. Клиенты
              видят 500.
            </p>
          </div>
          <div className="problem-card">
            <span className="emoji">🧠</span>
            <h3>Расписание = забыл</h3>
            <p>
              Cron на 3:00 ночи. Сервер упал в 2:59. Бекап не успел. Данные
              потеряны навсегда.
            </p>
          </div>
        </div>

        <div className="solution-box">
          <h3>FlowLink = Undo для продакшена</h3>
          <p>
            Перед каждой опасной командой — мгновенный снапшот только затронутых
            файлов. Килобайты вместо гигабайт. Восстановление за секунды.
          </p>
        </div>
      </section>

      {/* ═══ COMPARISON — why not regular backup ═══ */}
      <section className="container">
        <h2>FlowLink vs Обычный бекап</h2>
        <p className="section-sub">
          Целевое снапшотирование вместо полного образа диска
        </p>

        <table className="comparison-table">
          <thead>
            <tr>
              <th></th>
              <th className="fl">FlowLink</th>
              <th className="old">Обычный бекап</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td>Размер снапшота</td>
              <td className="fl">10KB — 50MB</td>
              <td className="old">10 — 100 GB</td>
            </tr>
            <tr>
              <td>Время создания</td>
              <td className="fl">Миллисекунды</td>
              <td className="old">Минуты — часы</td>
            </tr>
            <tr>
              <td>Восстановление</td>
              <td className="fl">Секунды</td>
              <td className="old">Часы</td>
            </tr>
            <tr>
              <td>Что хранит</td>
              <td className="fl">Только уязвимые файлы</td>
              <td className="old">Всё (включая мусор)</td>
            </tr>
            <tr>
              <td>Когда бекапит</td>
              <td className="fl">Перед опасной командой</td>
              <td className="old">По расписанию</td>
            </tr>
            <tr>
              <td>Автоматический</td>
              <td className="fl">✓ AI определяет</td>
              <td className="old">~ Ручной cron</td>
            </tr>
            <tr>
              <td>E2EE</td>
              <td className="fl">✓ X25519 + AES-256</td>
              <td className="old">~ Зависит от решения</td>
            </tr>
          </tbody>
        </table>
      </section>

      {/* ═══ CHAPTER 2: HOW IT WORKS ═══ */}
      <section className="container" id="how">
        <h2>Как это работает</h2>
        <p className="section-sub">Три шага — и твой сервер под защитой</p>

        <div className="steps">
          <div className="step">
            <div className="step-number">1</div>
            <h3>Установил агента</h3>
            <p>Одна строка — и FlowLink на сервере</p>
            <span className="step-code">curl -fsSL ... | bash</span>
          </div>
          <div className="step">
            <div className="step-number">2</div>
            <h3>AI бекапит автоматически</h3>
            <p>
              Policy Engine перехватывает опасные команды (50+ паттернов) и
              создаёт targeted snapshot
            </p>
          </div>
          <div className="step">
            <div className="step-number">3</div>
            <h3>Восстановил за секунды</h3>
            <p>
              Что-то сломалось? Одна команда — и всё на месте
            </p>
            <span className="step-code">flowlink undo</span>
          </div>
        </div>
      </section>

      {/* ═══ FEATURES — Bento Grid ═══ */}
      <section className="container" id="features">
        <h2>Возможности</h2>
        <p className="section-sub">
          Всё что нужно для безопасного управления серверами
        </p>

        <div className="features-grid">
          <div className="feature-card">
            <span className="icon">⏪</span>
            <h3>Targeted Undo</h3>
            <p>
              Бекапит НЕ весь VPS, а только файлы под угрозой. Килобайты вместо
              гигабайт. Восстановление за секунды.
            </p>
            <span className="tag tag-green">Core</span>
          </div>

          <div className="feature-card">
            <span className="icon">🤖</span>
            <h3>AI Policy Engine</h3>
            <p>
              Автоматически определяет опасные команды: rm, DROP, systemctl,
              docker rm, git reset — 50+ паттернов в 4 категориях.
            </p>
            <span className="tag tag-blue">AI-native</span>
          </div>

          <div className="feature-card">
            <span className="icon">🔇</span>
            <h3>Set &amp; Forget</h3>
            <p>
              После первичной настройки — не надо помнить IP, ключи,
              расписания. Агент работает автономно на каждом сервере.
            </p>
            <span className="tag tag-green">Zero-ops</span>
          </div>

          <div className="feature-card">
            <span className="icon">🔐</span>
            <h3>E2EE Encryption</h3>
            <p>
              X25519 + AES-256-GCM. Приватные ключи только на агенте. Даже
              relay не может расшифровать данные.
            </p>
            <span className="tag tag-amber">Security</span>
          </div>

          <div className="feature-card">
            <span className="icon">📱</span>
            <h3>Telegram Control</h3>
            <p>
              Управляй серверами из Telegram: выполнение команд, просмотр
              статуса, undo — всё из чата.
            </p>
          </div>

          <div className="feature-card">
            <span className="icon">🛡️</span>
            <h3>Sandbox + Approval</h3>
            <p>
              Ограничение команд, путей и таймаутов. Опасные команды требуют
              подтверждения перед выполнением.
            </p>
            <span className="tag tag-amber">Security</span>
          </div>

          <div className="feature-card">
            <span className="icon">📋</span>
            <h3>Audit Log</h3>
            <p>
              HMAC-верифицированный лог всех команд. Кто, что, когда — полная
              прозрачность и compliance.
            </p>
            <span className="tag tag-blue">Compliance</span>
          </div>

          <div className="feature-card">
            <span className="icon">🚀</span>
            <h3>One-line Install</h3>
            <p>
              curl | bash — и агент на сервере. Ubuntu, Debian, CentOS, Arch.
              Автообновление включено.
            </p>
          </div>

          <div className="feature-card">
            <span className="icon">🖥️</span>
            <h3>Web Dashboard</h3>
            <p>
              Тёмная тема, мониторинг агентов, просмотр снапшотов, биллинг —
              всё в одном интерфейсе.
            </p>
          </div>
        </div>
      </section>

      {/* ═══ WHO IS IT FOR ═══ */}
      <section className="container">
        <h2>Кому нужен FlowLink</h2>
        <p className="section-sub">
          Если у тебя есть хотя бы один сервер — тебе это нужно
        </p>

        <div className="audience-grid">
          <div className="audience-card">
            <span className="emoji">👨‍💻</span>
            <h3>Девелоперы</h3>
            <p>Фрилансеры с 3-20 VPS. Боты, сайты, API</p>
          </div>
          <div className="audience-card">
            <span className="emoji">🔧</span>
            <h3>DevOps-команды</h3>
            <p>1-5 человек. Стандарт, аудит, approval</p>
          </div>
          <div className="audience-card">
            <span className="emoji">🤖</span>
            <h3>Владельцы ботов</h3>
            <p>TG-боты на VPS. Управление из Telegram</p>
          </div>
          <div className="audience-card">
            <span className="emoji">🏢</span>
            <h3>SaaS</h3>
            <p>Self-hosted проекты. Undo для деплоев</p>
          </div>
        </div>
      </section>

      {/* ═══ PRICING ═══ */}
      <section className="container" id="pricing">
        <h2>Тарифы</h2>
        <p className="section-sub">Начни бесплатно — масштабируйся когда нужно</p>

        <div className="pricing-grid">
          <div className="pricing-card">
            <h3>Free</h3>
            <div className="price">0 ₽</div>
            <div className="price-note">навсегда</div>
            <ul>
              <li>1 сервер</li>
              <li>Unlimited undo</li>
              <li>Policy Engine</li>
              <li>Community support</li>
            </ul>
          </div>

          <div className="pricing-card">
            <h3>Starter</h3>
            <div className="price">990 ₽</div>
            <div className="price-note">/мес</div>
            <ul>
              <li>3 сервера</li>
              <li>Telegram bot</li>
              <li>Web dashboard</li>
              <li>Email support</li>
            </ul>
          </div>

          <div className="pricing-card featured">
            <h3>Business</h3>
            <div className="price">4 990 ₽</div>
            <div className="price-note">/мес</div>
            <ul>
              <li>25 серверов</li>
              <li>E2EE encryption</li>
              <li>Audit log + HMAC</li>
              <li>Priority support</li>
              <li>Approval workflow</li>
            </ul>
          </div>

          <div className="pricing-card">
            <h3>Enterprise</h3>
            <div className="price">Custom</div>
            <div className="price-note">свяжитесь с нами</div>
            <ul>
              <li>Unlimited серверов</li>
              <li>Self-hosted relay</li>
              <li>SLA 99.9%</li>
              <li>Dedicated support</li>
              <li>Custom integrations</li>
            </ul>
          </div>
        </div>
      </section>

      {/* ═══ FAQ ═══ */}
      <section className="container" id="faq">
        <h2>FAQ</h2>
        <p className="section-sub">Частые вопросы</p>

        <div className="faq-list">
          <div className="faq-item">
            <h3>Чем FlowLink отличается от Time Machine / Borg / Restic?</h3>
            <p>
              FlowLink бекапит НЕ весь диск, а только файлы под угрозой от
              конкретной команды. Типичный снапшот — 10KB-50MB вместо 10-100GB.
              Бекап создаётся автоматически перед опасной командой, а не по
              расписанию. Восстановление — секунды, а не часы.
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
            <h3>Сколько места занимают снапшоты?</h3>
            <p>
              Typical snapshot — 10KB-50MB (tar.gz только затронутых файлов).
              Автоматическая очистка: retention 7 дней, max 50 снапшотов, 5GB
              общий лимит. Всё настраивается.
            </p>
          </div>

          <div className="faq-item">
            <h3>Что если агент упадёт?</h3>
            <p>
              Снапшоты хранятся локально на сервере в ~/.flowlink/backups/.
              Даже если агент недоступен, все бекапы на месте. Восстановление
              вручную: tar -xzf snapshot.tar.gz.
            </p>
          </div>

          <div className="faq-item">
            <h3>Какие команды считаются опасными?</h3>
            <p>
              50+ паттернов в 4 категориях: system_destroy (rm -rf /, mkfs),
              data_destroy (DROP TABLE, git reset --hard), service_disrupt
              (systemctl stop, docker rm), security_bypass (chmod 777).
            </p>
          </div>

          <div className="faq-item">
            <h3>Можно ли использовать без relay?</h3>
            <p>
              Да. Free-план работает локально с одним сервером. Relay нужен для
              multi-server управления, Telegram-бота и web dashboard.
            </p>
          </div>
        </div>
      </section>

      {/* ═══ INSTALL ═══ */}
      <section className="container" id="install">
        <h2>Быстрый старт</h2>
        <p className="section-sub">Одна команда — и твой сервер под защитой</p>

        <div className="install-block">
          <div className="terminal-topbar">
            <span className="terminal-dot red"></span>
            <span className="terminal-dot yellow"></span>
            <span className="terminal-dot green"></span>
            <span className="terminal-title">~ install flowlink</span>
          </div>
          <div className="terminal-body">
            <code>
              curl -fsSL
              https://raw.githubusercontent.com/braincreator/flowlink/main/scripts/install.sh
              | bash
            </code>
            <CopyButton
              text="curl -fsSL https://raw.githubusercontent.com/braincreator/flowlink/main/scripts/install.sh | bash"
            />
          </div>
        </div>
        <p className="install-note">
          Ubuntu · Debian · CentOS · Arch Linux · Автообновление включено
        </p>
      </section>

      {/* ═══ FOOTER ═══ */}
      <footer className="container">
        <p>
          <a
            href="https://github.com/braincreator/flowlink"
            target="_blank"
            rel="noopener noreferrer"
          >
            GitHub
          </a>{" "}
          · MIT License · FlowMasters © 2026
        </p>
      </footer>
    </div>
  );
}
