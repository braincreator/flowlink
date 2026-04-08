import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState, useEffect, createContext, useContext } from 'react';
import { NavLink, Outlet, useLocation } from 'react-router-dom';
import { useKeyboardShortcuts } from '../hooks/useKeyboardShortcuts';
import { useTranslation } from 'react-i18next';
import { LayoutDashboard, Bot, Shield, FileText, MonitorPlay, HardDrive, FileCode, Smartphone, Users, BarChart3, Settings, ChevronLeft, ChevronRight, Menu, X, Brain, Puzzle, CreditCard, Sun, Moon, Globe, GraduationCap, TerminalSquare, Radio } from 'lucide-react';
import { version } from '../../package.json';
const navGroups = [
    {
        label: 'Main',
        items: [
            { to: '/', icon: LayoutDashboard, labelKey: 'nav.dashboard' },
            { to: '/agents', icon: Bot, labelKey: 'nav.agents' },
            { to: '/shield', icon: Shield, labelKey: 'nav.shield' },
            { to: '/devices', icon: Smartphone, labelKey: 'nav.devices' },
        ],
    },
    {
        label: 'Monitoring',
        items: [
            { to: '/audit', icon: FileText, labelKey: 'nav.audit' },
            { to: '/sessions', icon: MonitorPlay, labelKey: 'nav.sessions' },
            { to: '/metrics', icon: BarChart3, labelKey: 'nav.metrics' },
            { to: '/backups', icon: HardDrive, labelKey: 'nav.backups' },
        ],
    },
    {
        label: 'Security',
        items: [
            { to: '/rbac', icon: Users, labelKey: 'nav.rbac' },
            { to: '/policies', icon: FileCode, labelKey: 'nav.policies' },
        ],
    },
    {
        label: 'Integrations',
        items: [
            { to: '/llm', icon: Brain, labelKey: 'nav.llm' },
            { to: '/mcp', icon: Puzzle, labelKey: 'nav.mcp' },
            { to: '/billing', icon: CreditCard, labelKey: 'nav.billing' },
        ],
    },
    {
        label: 'Terminal',
        items: [
            { to: '/terminal', icon: TerminalSquare, labelKey: 'nav.terminal' },
            { to: '/terminal/soc', icon: LayoutDashboard, labelKey: 'nav.terminal_soc' },
            { to: '/terminal/relay', icon: Radio, labelKey: 'nav.terminal_relay' },
        ],
    },
    {
        items: [
            { to: '/settings', icon: Settings, labelKey: 'nav.settings' },
            { to: '/onboarding', icon: GraduationCap, labelKey: 'nav.onboarding' },
        ],
    },
];
const titleKeys = {};
navGroups.forEach(g => g.items.forEach(n => {
    titleKeys[n.to] = n.to === '/' ? 'nav.dashboard' : n.labelKey;
}));
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
    useKeyboardShortcuts();
    useEffect(() => {
        document.documentElement.setAttribute('data-theme', theme);
        localStorage.setItem('flowlink_theme', theme);
    }, [theme]);
    const toggleTheme = () => {
        document.documentElement.setAttribute('data-transitioning', '');
        setTheme(t => t === 'dark' ? 'light' : 'dark');
        setTimeout(() => document.documentElement.removeAttribute('data-transitioning'), 250);
    };
    const isDark = theme === 'dark';
    const title = t(titleKeys[location.pathname] || 'nav.dashboard');
    return (_jsx(SidebarContext.Provider, { value: { collapsed, setCollapsed, mobileOpen, setMobileOpen }, children: _jsxs("div", { className: "flex h-screen overflow-hidden bg-[var(--color-bg)]", children: [_jsx("button", { onClick: () => setMobileOpen(true), "aria-label": "Open menu", className: "fixed top-3 left-3 z-[60] flex h-10 w-10 items-center justify-center rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)] text-[var(--color-text)] lg:hidden", children: _jsx(Menu, { size: 18 }) }), mobileOpen && _jsx("div", { className: "fixed inset-0 z-[70] bg-black/50 lg:hidden", onClick: () => setMobileOpen(false) }), _jsxs("aside", { className: `fixed inset-y-0 left-0 z-[80] flex flex-col border-r border-[var(--color-border)] bg-[var(--color-surface)] transition-all duration-300 md:static md:z-auto
          ${collapsed ? 'w-[68px]' : 'w-[240px]'}
          ${mobileOpen ? 'translate-x-0' : '-translate-x-full lg:translate-x-0'}`, children: [_jsxs("div", { className: `flex h-16 items-center border-b border-[var(--color-border)] ${collapsed ? 'justify-center px-2' : 'px-5'}`, children: [_jsxs("div", { className: "flex items-center gap-2.5", children: [_jsx("div", { className: "flex h-8 w-8 items-center justify-center rounded-lg bg-gradient-to-br from-indigo-500 to-indigo-600 text-sm font-bold text-white", children: "\u26A1" }), !collapsed && _jsxs("span", { className: "text-base font-bold tracking-tight", children: [_jsx("span", { className: "text-[var(--color-accent-light)]", children: "Flow" }), _jsx("span", { className: "text-[var(--color-text)]", children: "Link" })] })] }), _jsx("button", { onClick: () => setMobileOpen(false), className: "ml-auto lg:hidden", children: _jsx(X, { size: 18 }) })] }), _jsx("nav", { className: "flex-1 overflow-y-auto py-3", children: navGroups.map((group, groupIdx) => (_jsxs("div", { children: [groupIdx > 0 && (_jsx("div", { className: `${collapsed ? 'mx-3' : 'mx-4'} mt-4 pt-3 border-t border-[var(--color-border)]`, children: !collapsed && group.label && (_jsx("span", { className: "text-[10px] font-semibold uppercase tracking-wider text-[var(--color-dim)]", children: group.label })) })), groupIdx === 0 && !collapsed && group.label && (_jsx("div", { className: "mx-4 mb-1", children: _jsx("span", { className: "text-[10px] font-semibold uppercase tracking-wider text-[var(--color-dim)]", children: group.label }) })), group.items.map(item => (_jsxs(NavLink, { to: item.to, end: item.to === '/', onClick: () => setMobileOpen(false), title: collapsed ? t(item.labelKey) : undefined, className: ({ isActive }) => `flex items-center gap-3 mx-2 rounded-lg px-3 py-2.5 text-sm font-medium transition-all duration-150
                      ${isActive ? 'bg-[var(--color-accent)]/15 text-[var(--color-accent-light)]' : 'text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)]'}
                      ${collapsed ? 'justify-center' : ''}`, children: [_jsx(item.icon, { size: 18, className: "flex-shrink-0" }), !collapsed && _jsx("span", { children: t(item.labelKey) })] }, item.to)))] }, group.label ?? `ungrouped-${groupIdx}`))) }), _jsx("div", { className: "hidden border-t border-[var(--color-border)] p-3 lg:block", children: _jsx("button", { onClick: () => setCollapsed(!collapsed), "aria-label": collapsed ? "Expand sidebar" : "Collapse sidebar", className: "flex w-full items-center justify-center rounded-lg py-2 text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors", children: collapsed ? _jsx(ChevronRight, { size: 16 }) : _jsx(ChevronLeft, { size: 16 }) }) })] }), _jsxs("div", { className: "flex flex-1 flex-col overflow-hidden", children: [_jsxs("header", { className: "flex h-16 items-center justify-between border-b border-[var(--color-border)] bg-[var(--color-surface)]/50 backdrop-blur-md px-6 lg:pl-6 pl-14", children: [_jsx("h1", { className: "text-lg font-semibold", children: title }), _jsxs("div", { className: "flex items-center gap-3", children: [_jsx(LanguageToggle, {}), _jsx("button", { onClick: toggleTheme, className: "flex h-8 w-8 items-center justify-center rounded-lg border border-[var(--color-border)] text-[var(--color-dim)] hover:bg-[var(--color-surface2)] hover:text-[var(--color-text)] transition-colors", title: "Toggle theme", "aria-label": isDark ? 'Switch to light mode' : 'Switch to dark mode', children: theme === 'dark' ? _jsx(Sun, { size: 16 }) : _jsx(Moon, { size: 16 }) }), _jsx("span", { className: "hidden text-xs font-medium text-[var(--color-dim)] sm:inline", children: t(`settings.${theme}`) }), _jsxs("kbd", { className: "hidden lg:inline-flex items-center gap-0.5 rounded border border-[var(--color-border)] bg-[var(--color-bg)] px-1.5 py-0.5 text-[10px] text-[var(--color-dim)] font-mono", children: [_jsx("span", { children: "\u2318" }), "K"] }), _jsx("div", { className: "h-2 w-2 rounded-full bg-emerald-400 pulse-dot" }), _jsxs("span", { className: "text-xs text-[var(--color-dim)]", children: ["v", version] }), _jsx("div", { className: "ml-2 flex h-8 w-8 items-center justify-center rounded-full bg-gradient-to-br from-indigo-500 to-purple-600 text-xs font-bold text-white", children: "A" })] })] }), _jsx("main", { className: "flex-1 overflow-y-auto p-6", children: _jsx(Outlet, {}) })] })] }) }));
}
