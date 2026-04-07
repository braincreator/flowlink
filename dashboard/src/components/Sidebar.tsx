import { useState, useEffect, createContext, useContext, type ReactNode } from 'react';
import { NavLink, Outlet, useLocation } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import {
  LayoutDashboard, Bot, Shield, FileText, MonitorPlay, HardDrive,
  FileCode, Smartphone, Users, BarChart3, Settings, ChevronLeft, ChevronRight, Menu, X, Brain, Puzzle, CreditCard, Sun, Moon, Globe, GraduationCap, TerminalSquare
} from 'lucide-react';

const navItems = [
  { to: '/', icon: LayoutDashboard, labelKey: 'nav.dashboard' },
  { to: '/agents', icon: Bot, labelKey: 'nav.agents' },
  { to: '/shield', icon: Shield, labelKey: 'nav.shield' },
  { to: '/audit', icon: FileText, labelKey: 'nav.audit' },
  { to: '/sessions', icon: MonitorPlay, labelKey: 'nav.sessions' },
  { to: '/backups', icon: HardDrive, labelKey: 'nav.backups' },
  { to: '/policies', icon: FileCode, labelKey: 'nav.policies' },
  { to: '/devices', icon: Smartphone, labelKey: 'nav.devices' },
  { to: '/rbac', icon: Users, labelKey: 'nav.rbac' },
  { to: '/metrics', icon: BarChart3, labelKey: 'nav.metrics' },
  { to: '/settings', icon: Settings, labelKey: 'nav.settings' },
  { to: '/llm', icon: Brain, labelKey: 'nav.llm' },
  { to: '/mcp', icon: Puzzle, labelKey: 'nav.mcp' },
  { to: '/billing', icon: CreditCard, labelKey: 'nav.billing' },
  { to: '/onboarding', icon: GraduationCap, labelKey: 'nav.onboarding' },
  { to: '/terminal', icon: TerminalSquare, labelKey: 'nav.terminal' },
];

const titleKeys: Record<string, string> = {};
navItems.forEach(n => { if (n.to !== '/') titleKeys[n.to] = n.labelKey; });
titleKeys['/'] = 'nav.dashboard';

interface SidebarCtx {
  collapsed: boolean;
  setCollapsed: (v: boolean) => void;
  mobileOpen: boolean;
  setMobileOpen: (v: boolean) => void;
}

const SidebarContext = createContext<SidebarCtx>({ collapsed: false, setCollapsed: () => {}, mobileOpen: false, setMobileOpen: () => {} });
export const useSidebar = () => useContext(SidebarContext);

function LanguageToggle() {
  const { i18n } = useTranslation();
  const toggle = () => {
    const next = i18n.language === 'ru' ? 'en' : 'ru';
    i18n.changeLanguage(next);
    localStorage.setItem('flowlink_lang', next);
  };
  return (
    <button onClick={toggle}
      className="flex h-8 w-8 items-center justify-center rounded-lg border border-[var(--color-border)] text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors"
      title={i18n.language === 'ru' ? 'Switch to English' : 'Переключить на русский'}>
      <Globe size={16} />
    </button>
  );
}

export function Layout() {
  const [collapsed, setCollapsed] = useState(false);
  const [mobileOpen, setMobileOpen] = useState(false);
  const [theme, setTheme] = useState<'dark' | 'light'>(() => {
    const stored = localStorage.getItem('flowlink_theme') as 'dark' | 'light' | null;
    if (stored) return stored;
    return window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
  });
  const location = useLocation();
  const { t } = useTranslation();

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('flowlink_theme', theme);
  }, [theme]);

  const toggleTheme = () => {
    document.documentElement.setAttribute('data-transitioning', '');
    setTheme(t => t === 'dark' ? 'light' : 'dark');
    setTimeout(() => document.documentElement.removeAttribute('data-transitioning'), 250);
  };

  const title = t(titleKeys[location.pathname] || 'nav.dashboard');

  return (
    <SidebarContext.Provider value={{ collapsed, setCollapsed, mobileOpen, setMobileOpen }}>
      <div className="flex h-screen overflow-hidden bg-[var(--color-bg)]">
        {/* Mobile menu button */}
        <button onClick={() => setMobileOpen(true)}
          className="fixed top-3 left-3 z-[60] flex h-10 w-10 items-center justify-center rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] text-[var(--color-text)] lg:hidden">
          <Menu size={18} />
        </button>

        {/* Mobile backdrop */}
        {mobileOpen && <div className="fixed inset-0 z-[70] bg-black/50 lg:hidden" onClick={() => setMobileOpen(false)} />}

        {/* Sidebar */}
        <aside className={`fixed inset-y-0 left-0 z-[80] flex flex-col border-r border-[var(--color-border)] bg-[var(--color-surface)] transition-all duration-300 md:static md:z-auto
          ${collapsed ? 'w-[68px]' : 'w-[240px]'}
          ${mobileOpen ? 'translate-x-0' : '-translate-x-full lg:translate-x-0'}`}>
          {/* Logo */}
          <div className={`flex h-16 items-center border-b border-[var(--color-border)] ${collapsed ? 'justify-center px-2' : 'px-5'}`}>
            <div className="flex items-center gap-2.5">
              <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-gradient-to-br from-indigo-500 to-indigo-600 text-sm font-bold text-white">⚡</div>
              {!collapsed && <span className="text-base font-bold tracking-tight"><span className="text-[var(--color-accent-light)]">Flow</span><span className="text-[var(--color-text)]">Link</span></span>}
            </div>
            {/* Mobile close */}
            <button onClick={() => setMobileOpen(false)} className="ml-auto lg:hidden"><X size={18} /></button>
          </div>

          {/* Nav */}
          <nav className="flex-1 overflow-y-auto py-3">
            {navItems.map(item => (
              <NavLink key={item.to} to={item.to} end={item.to === '/'}
                onClick={() => setMobileOpen(false)}
                className={({ isActive }) => `flex items-center gap-3 mx-2 rounded-lg px-3 py-2.5 text-sm font-medium transition-all duration-150
                  ${isActive ? 'bg-[var(--color-accent)]/15 text-[var(--color-accent-light)]' : 'text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)]'}
                  ${collapsed ? 'justify-center' : ''}`}>
                <item.icon size={18} className="flex-shrink-0" />
                {!collapsed && <span>{t(item.labelKey)}</span>}
              </NavLink>
            ))}
          </nav>

          {/* Collapse toggle */}
          <div className="hidden border-t border-[var(--color-border)] p-3 lg:block">
            <button onClick={() => setCollapsed(!collapsed)}
              className="flex w-full items-center justify-center rounded-lg py-2 text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors">
              {collapsed ? <ChevronRight size={16} /> : <ChevronLeft size={16} />}
            </button>
          </div>
        </aside>

        {/* Main area */}
        <div className="flex flex-1 flex-col overflow-hidden">
          {/* Header */}
          <header className="flex h-16 items-center justify-between border-b border-[var(--color-border)] bg-[var(--color-surface)]/50 backdrop-blur-md px-6 lg:pl-6 pl-14">
            <h1 className="text-lg font-semibold">{title}</h1>
            <div className="flex items-center gap-3">
              <LanguageToggle />
              <button onClick={toggleTheme} className="flex h-8 w-8 items-center justify-center rounded-lg border border-[var(--color-border)] text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors" title="Toggle theme">
                {theme === 'dark' ? <Sun size={16} /> : <Moon size={16} />}
              </button>
              <span className="hidden text-xs font-medium text-[var(--color-dim)] sm:inline">{t(`settings.${theme}`)}</span>
              <div className="h-2 w-2 rounded-full bg-emerald-400 pulse-dot" />
              <span className="text-xs text-[var(--color-dim)]">v0.9.2</span>
              <div className="ml-2 flex h-8 w-8 items-center justify-center rounded-full bg-gradient-to-br from-indigo-500 to-purple-600 text-xs font-bold text-white">A</div>
            </div>
          </header>

          {/* Content */}
          <main className="flex-1 overflow-y-auto p-6">
            <Outlet />
          </main>
        </div>
      </div>
    </SidebarContext.Provider>
  );
}
