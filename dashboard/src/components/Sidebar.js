import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState, useEffect, createContext, useContext } from 'react';
import { NavLink, Outlet, useLocation } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { LayoutDashboard, Bot, Shield, FileText, MonitorPlay, HardDrive, FileCode, Smartphone, Users, BarChart3, Settings, ChevronLeft, ChevronRight, Menu, X, Brain, Puzzle, CreditCard, Sun, Moon, Globe, GraduationCap, TerminalSquare } from 'lucide-react';
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
const titleKeys = {};
navItems.forEach(n => { if (n.to !== '/')
    titleKeys[n.to] = n.labelKey; });
titleKeys['/'] = 'nav.dashboard';
const SidebarContext = createContext({ collapsed: false, setCollapsed: () => { }, mobileOpen: false, setMobileOpen: () => { } });
export const useSidebar = () => useContext(SidebarContext);
function LanguageToggle() {
    const { i18n } = useTranslation();
    const toggle = () => {
        const next = i18n.language === 'ru' ? 'en' : 'ru';
        i18n.changeLanguage(next);
        localStorage.setItem('flowlink_lang', next);
    };
    return (_jsx("button", { onClick: toggle, className: "flex h-8 w-8 items-center justify-center rounded-lg border border-[var(--color-border)] text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors", title: i18n.language === 'ru' ? 'Switch to English' : 'Переключить на русский', children: _jsx(Globe, { size: 16 }) }));
}
export function Layout() {
    const [collapsed, setCollapsed] = useState(false);
    const [mobileOpen, setMobileOpen] = useState(false);
    const [theme, setTheme] = useState(() => {
        const stored = localStorage.getItem('flowlink_theme');
        if (stored)
            return stored;
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
    return (_jsx(SidebarContext.Provider, { value: { collapsed, setCollapsed, mobileOpen, setMobileOpen }, children: _jsxs("div", { className: "flex h-screen overflow-hidden bg-[var(--color-bg)]", children: [_jsx("button", { onClick: () => setMobileOpen(true), className: "fixed top-3 left-3 z-[60] flex h-10 w-10 items-center justify-center rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] text-[var(--color-text)] lg:hidden", children: _jsx(Menu, { size: 18 }) }), mobileOpen && _jsx("div", { className: "fixed inset-0 z-[70] bg-black/50 lg:hidden", onClick: () => setMobileOpen(false) }), _jsxs("aside", { className: `fixed inset-y-0 left-0 z-[80] flex flex-col border-r border-[var(--color-border)] bg-[var(--color-surface)] transition-all duration-300 md:static md:z-auto
          ${collapsed ? 'w-[68px]' : 'w-[240px]'}
          ${mobileOpen ? 'translate-x-0' : '-translate-x-full lg:translate-x-0'}`, children: [_jsxs("div", { className: `flex h-16 items-center border-b border-[var(--color-border)] ${collapsed ? 'justify-center px-2' : 'px-5'}`, children: [_jsxs("div", { className: "flex items-center gap-2.5", children: [_jsx("div", { className: "flex h-8 w-8 items-center justify-center rounded-lg bg-gradient-to-br from-indigo-500 to-indigo-600 text-sm font-bold text-white", children: "\u26A1" }), !collapsed && _jsxs("span", { className: "text-base font-bold tracking-tight", children: [_jsx("span", { className: "text-[var(--color-accent-light)]", children: "Flow" }), _jsx("span", { className: "text-[var(--color-text)]", children: "Link" })] })] }), _jsx("button", { onClick: () => setMobileOpen(false), className: "ml-auto lg:hidden", children: _jsx(X, { size: 18 }) })] }), _jsx("nav", { className: "flex-1 overflow-y-auto py-3", children: navItems.map(item => (_jsxs(NavLink, { to: item.to, end: item.to === '/', onClick: () => setMobileOpen(false), className: ({ isActive }) => `flex items-center gap-3 mx-2 rounded-lg px-3 py-2.5 text-sm font-medium transition-all duration-150
                  ${isActive ? 'bg-[var(--color-accent)]/15 text-[var(--color-accent-light)]' : 'text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)]'}
                  ${collapsed ? 'justify-center' : ''}`, children: [_jsx(item.icon, { size: 18, className: "flex-shrink-0" }), !collapsed && _jsx("span", { children: t(item.labelKey) })] }, item.to))) }), _jsx("div", { className: "hidden border-t border-[var(--color-border)] p-3 lg:block", children: _jsx("button", { onClick: () => setCollapsed(!collapsed), className: "flex w-full items-center justify-center rounded-lg py-2 text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors", children: collapsed ? _jsx(ChevronRight, { size: 16 }) : _jsx(ChevronLeft, { size: 16 }) }) })] }), _jsxs("div", { className: "flex flex-1 flex-col overflow-hidden", children: [_jsxs("header", { className: "flex h-16 items-center justify-between border-b border-[var(--color-border)] bg-[var(--color-surface)]/50 backdrop-blur-md px-6 lg:pl-6 pl-14", children: [_jsx("h1", { className: "text-lg font-semibold", children: title }), _jsxs("div", { className: "flex items-center gap-3", children: [_jsx(LanguageToggle, {}), _jsx("button", { onClick: toggleTheme, className: "flex h-8 w-8 items-center justify-center rounded-lg border border-[var(--color-border)] text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors", title: "Toggle theme", children: theme === 'dark' ? _jsx(Sun, { size: 16 }) : _jsx(Moon, { size: 16 }) }), _jsx("span", { className: "hidden text-xs font-medium text-[var(--color-dim)] sm:inline", children: t(`settings.${theme}`) }), _jsx("div", { className: "h-2 w-2 rounded-full bg-emerald-400 pulse-dot" }), _jsx("span", { className: "text-xs text-[var(--color-dim)]", children: "v0.9.2" }), _jsx("div", { className: "ml-2 flex h-8 w-8 items-center justify-center rounded-full bg-gradient-to-br from-indigo-500 to-purple-600 text-xs font-bold text-white", children: "A" })] })] }), _jsx("main", { className: "flex-1 overflow-y-auto p-6", children: _jsx(Outlet, {}) })] })] }) }));
}
