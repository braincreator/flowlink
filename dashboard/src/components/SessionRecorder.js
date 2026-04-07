import { jsxs as _jsxs, jsx as _jsx, Fragment as _Fragment } from "react/jsx-runtime";
import { useState, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { X, Download, Send, Loader2 } from 'lucide-react';
import { useSessionRecorder } from '../hooks/useSessionRecorder';
import { themes, toXtermTheme } from './terminal/themes';
const speedOptions = [0.5, 1, 1.5, 2, 4];
function formatDuration(ms) {
    const s = Math.round(ms / 1000);
    const m = Math.floor(s / 60);
    const sec = s % 60;
    return m > 0 ? `${m}m ${sec}s` : `${sec}s`;
}
export default function SessionRecorder({ open, onClose, session }) {
    const { t } = useTranslation();
    const { recordSession, downloadRecording, shareToTelegram, recording, progress, cancelRecording } = useSessionRecorder();
    const [themeId, setThemeId] = useState('flowlink-dark');
    const [speed, setSpeed] = useState(1);
    const [result, setResult] = useState(null);
    const [error, setError] = useState(null);
    const theme = useMemo(() => themes.find(th => th.id === themeId), [themeId]);
    const xtermTheme = useMemo(() => theme ? toXtermTheme(theme) : undefined, [theme]);
    useEffect(() => {
        if (open) {
            setResult(null);
            setError(null);
        }
    }, [open]);
    const handleRecord = async () => {
        if (!session?.castData)
            return;
        setError(null);
        try {
            const res = await recordSession(session.castData, {
                speed,
                theme: xtermTheme,
                fontSize: 15,
            });
            setResult(res);
        }
        catch (err) {
            setError(err.message || 'Recording failed');
        }
    };
    if (!open || !session)
        return null;
    return (_jsx("div", { className: "fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm", children: _jsxs("div", { className: "w-full max-w-lg rounded-2xl border border-white/10 bg-[var(--color-surface)] shadow-2xl", children: [_jsxs("div", { className: "flex items-center justify-between border-b border-white/[0.06] px-5 py-4", children: [_jsxs("h3", { className: "text-base font-semibold", children: ["\uD83C\uDFAC ", t('sessions.record_session')] }), _jsx("button", { onClick: onClose, className: "rounded-lg p-1.5 text-[var(--color-dim)] hover:bg-white/5", children: _jsx(X, { size: 16 }) })] }), _jsxs("div", { className: "space-y-5 p-5", children: [_jsxs("div", { className: "rounded-xl bg-white/[0.03] border border-white/[0.06] px-4 py-3", children: [_jsx("div", { className: "text-sm font-medium", children: session.hostname || session.id }), _jsxs("div", { className: "mt-1 text-xs text-[var(--color-dim)]", children: [formatDuration(session.duration_ms || 0), " \u00B7 ", session.commands_count || 0, " commands"] })] }), _jsxs("div", { children: [_jsx("label", { className: "mb-2 block text-xs font-medium uppercase tracking-wider text-[var(--color-dim)]", children: t('sessions.theme') }), _jsx("div", { className: "flex flex-wrap gap-2", children: themes.map((th) => (_jsx("button", { onClick: () => setThemeId(th.id), className: `h-8 w-8 rounded-lg border-2 transition-all ${themeId === th.id
                                            ? 'border-[var(--color-accent)] scale-110'
                                            : 'border-transparent opacity-60 hover:opacity-100'}`, style: { background: th.colors?.background || '#0d1117' }, title: th.name }, th.id))) })] }), _jsxs("div", { children: [_jsx("label", { className: "mb-2 block text-xs font-medium uppercase tracking-wider text-[var(--color-dim)]", children: t('sessions.speed') }), _jsx("div", { className: "flex gap-2", children: speedOptions.map(s => (_jsxs("button", { onClick: () => setSpeed(s), className: `rounded-lg px-3 py-1.5 text-xs font-medium transition-all ${speed === s
                                            ? 'bg-[var(--color-accent)] text-white'
                                            : 'bg-white/5 text-[var(--color-dim)] hover:bg-white/10'}`, children: [s, "x"] }, s))) })] }), !result && (_jsxs("div", { className: "space-y-3", children: [_jsx("button", { onClick: recording ? cancelRecording : handleRecord, disabled: recording, className: "flex w-full items-center justify-center gap-2 rounded-xl bg-[var(--color-accent)] px-4 py-3 text-sm font-semibold text-white transition-all hover:bg-[var(--color-accent-light)] disabled:opacity-60", children: recording ? (_jsxs(_Fragment, { children: [_jsx(Loader2, { size: 16, className: "animate-spin" }), t('sessions.recording'), " ", progress, "%"] })) : (`▶ ${t('sessions.record_session')}`) }), recording && (_jsx("div", { className: "h-1.5 overflow-hidden rounded-full bg-white/5", children: _jsx("div", { className: "h-full rounded-full bg-[var(--color-accent)] transition-all duration-300", style: { width: `${progress}%` } }) }))] })), error && (_jsx("div", { className: "rounded-xl bg-red-500/10 border border-red-500/20 px-4 py-3 text-sm text-red-400", children: error })), result && (_jsxs("div", { className: "space-y-4", children: [_jsx("video", { src: result.url, controls: true, autoPlay: true, loop: true, className: "w-full rounded-xl border border-white/[0.06] bg-black" }), _jsxs("div", { className: "flex gap-3", children: [_jsxs("button", { onClick: () => downloadRecording(result, `session-${session.id.slice(0, 8)}.webm`), className: "flex flex-1 items-center justify-center gap-2 rounded-xl bg-white/5 px-4 py-2.5 text-sm font-medium hover:bg-white/10 transition-colors", children: [_jsx(Download, { size: 15 }), " ", t('sessions.download')] }), _jsxs("button", { onClick: () => shareToTelegram(result), className: "flex flex-1 items-center justify-center gap-2 rounded-xl bg-[#2AABEE]/15 px-4 py-2.5 text-sm font-medium text-[#2AABEE] hover:bg-[#2AABEE]/25 transition-colors", children: [_jsx(Send, { size: 15 }), " ", t('sessions.share_telegram')] })] })] }))] })] }) }));
}
