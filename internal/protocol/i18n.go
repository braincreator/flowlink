package protocol

import (
	"fmt"
	"sync"
)

// =============================================================================
// i18n — minimal localization for protocol messages.
// Default locale: English. Russian available via SetLocale("ru").
// Thread-safe. Lives in protocol package to avoid import cycles.
// =============================================================================

var (
	i18nMu    sync.RWMutex
	i18nLocale = "ru"
	i18nDicts  = map[string]map[string]string{
		"en": enMessages,
		"ru": ruMessages,
	}
)

// SetLocale changes the active locale. Affects all subsequent T() calls.
func SetLocale(lang string) {
	i18nMu.Lock()
	defer i18nMu.Unlock()
	i18nLocale = lang
}

// GetLocale returns the current locale.
func GetLocale() string {
	i18nMu.RLock()
	defer i18nMu.RUnlock()
	return i18nLocale
}

// RegisterLocale adds or replaces a locale dictionary.
func RegisterLocale(lang string, messages map[string]string) {
	i18nMu.Lock()
	defer i18nMu.Unlock()
	i18nDicts[lang] = messages
}

// T returns the localized message for a code.
// Falls back to the code itself if no translation exists.
func T(code string) string {
	i18nMu.RLock()
	defer i18nMu.RUnlock()
	if msgs, ok := i18nDicts[i18nLocale]; ok {
		if msg, ok := msgs[code]; ok {
			return msg
		}
	}
	// Fallback to English
	if msgs, ok := i18nDicts["en"]; ok {
		if msg, ok := msgs[code]; ok {
			return msg
		}
	}
	return code
}

// Tf returns a formatted localized message (printf-style).
func Tf(code string, args ...any) string {
	return fmt.Sprintf(T(code), args...)
}
